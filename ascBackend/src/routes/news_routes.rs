// 资讯（News）路由：原前端 src/data/news/*.md 迁入数据库
//
//   GET    /api/news                 资讯列表（仅已发布，不含正文）
//   GET    /api/news/{slug}          资讯详情（管理员可预览未发布）
//   GET    /api/admin/news           全部资讯（管理员，含未发布）
//   POST   /api/admin/news           新建资讯（管理员）
//   PUT    /api/admin/news/{id}      更新资讯（管理员）
//   DELETE /api/admin/news/{id}      删除资讯（管理员）
use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::Value;

use crate::auth::AdminUser;
use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::News;
use crate::schema::news;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/news", get(list_published))
        .route("/api/news/{slug}", get(news_detail))
        .route("/api/admin/news", get(admin_list).post(admin_create))
        .route(
            "/api/admin/news/{id}",
            put(admin_update).delete(admin_delete),
        )
}

// ============ 校验 ============

/// slug 只允许小写字母、数字、中划线，且不能以中划线开头/结尾
fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

#[derive(serde::Deserialize)]
struct NewsInput {
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    date: String,
    content: String,
    /// 省略时为 None：创建默认发布，更新时保留原状态
    #[serde(default)]
    published: Option<bool>,
}

fn validate(input: &NewsInput) -> Result<(), AppError> {
    let slug = input.slug.trim();
    if !valid_slug(slug) {
        return Err(AppError::bad_request(
            "slug 只能包含小写字母、数字、中划线，且不能以中划线开头或结尾",
        ));
    }
    if input.title.trim().is_empty() {
        return Err(AppError::bad_request("标题不能为空"));
    }
    if input.title.chars().count() > 200 {
        return Err(AppError::bad_request("标题过长（最多 200 字）"));
    }
    if input.date.trim().is_empty() {
        return Err(AppError::bad_request("日期不能为空"));
    }
    if input.content.trim().is_empty() {
        return Err(AppError::bad_request("正文不能为空"));
    }
    normalize_date(&input.date)?;
    Ok(())
}

/// 日期统一为 YYYY-MM-DD（列表按 date 字符串排序，格式必须一致）
fn normalize_date(raw: &str) -> Result<String, AppError> {
    let normalized = raw.trim().replace('/', "-");
    let date = chrono::NaiveDate::parse_from_str(&normalized, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request("日期格式必须为 YYYY-MM-DD"))?;
    Ok(date.format("%Y-%m-%d").to_string())
}

// ============ GET /api/news ============

async fn list_published(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let items = run_db(&state.pool, |conn| {
        news::table
            .filter(news::published.eq(true))
            .order((news::date.desc(), news::id.desc()))
            .load::<News>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let list: Vec<Value> = items.iter().map(|n| n.to_list_json()).collect();
    Ok(ok(Value::Array(list)))
}

// ============ GET /api/news/{slug} ============

async fn news_detail(
    State(state): State<AppState>,
    admin: Option<AdminUser>,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    let item = run_db(&state.pool, move |conn| {
        news::table
            .filter(news::slug.eq(slug))
            .first::<News>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;
    let item = item.ok_or_else(|| AppError::not_found("资讯不存在"))?;
    // 未发布内容仅管理员可见（admin 为 None 时是匿名/普通用户）
    if !item.published && admin.is_none() {
        return Err(AppError::not_found("资讯不存在"));
    }
    Ok(ok(item.to_json()))
}

// ============ GET /api/admin/news ============

async fn admin_list(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Value>> {
    let items = run_db(&state.pool, |conn| {
        news::table
            .order((news::date.desc(), news::id.desc()))
            .load::<News>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let list: Vec<Value> = items.iter().map(|n| n.to_list_json()).collect();
    Ok(ok(Value::Array(list)))
}

// ============ POST /api/admin/news ============

async fn admin_create(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(input): Json<NewsInput>,
) -> ApiResult<Json<Value>> {
    validate(&input)?;
    let slug = input.slug.trim().to_string();
    let exists = run_db(&state.pool, move |conn| {
        news::table
            .filter(news::slug.eq(slug))
            .first::<News>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;
    if exists.is_some() {
        return Err(AppError::conflict("该 slug 已存在"));
    }

    let date = normalize_date(&input.date)?;
    let now = Utc::now().naive_utc();
    let created = run_db(&state.pool, move |conn| {
        diesel::insert_into(news::table)
            .values((
                news::slug.eq(input.slug.trim()),
                news::title.eq(input.title.trim()),
                news::description.eq(input.description.trim()),
                news::date.eq(date),
                news::content.eq(&input.content),
                news::published.eq(input.published.unwrap_or(true)),
                news::created_at.eq(now),
                news::updated_at.eq(now),
            ))
            .get_result::<News>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok_msg("资讯已创建", created.to_json()))
}

// ============ PUT /api/admin/news/{id} ============

async fn admin_update(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i32>,
    Json(input): Json<NewsInput>,
) -> ApiResult<Json<Value>> {
    validate(&input)?;
    let date = normalize_date(&input.date)?;
    let now = Utc::now().naive_utc();
    let updated = run_db(&state.pool, move |conn| {
        let existing = news::table
            .find(id)
            .first::<News>(conn)
            .optional()
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("资讯不存在"))?;

        let slug = input.slug.trim().to_string();
        let conflict = news::table
            .filter(news::slug.eq(&slug))
            .filter(news::id.ne(id))
            .first::<News>(conn)
            .optional()
            .map_err(AppError::from)?;
        if conflict.is_some() {
            return Err(AppError::conflict("该 slug 已存在"));
        }

        diesel::update(news::table.find(id))
            .set((
                news::slug.eq(slug),
                news::title.eq(input.title.trim()),
                news::description.eq(input.description.trim()),
                news::date.eq(date),
                news::content.eq(&input.content),
                news::published.eq(input.published.unwrap_or(existing.published)),
                news::updated_at.eq(now),
            ))
            .get_result::<News>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok_msg("资讯已更新", updated.to_json()))
}

// ============ DELETE /api/admin/news/{id} ============

async fn admin_delete(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let rows = run_db(&state.pool, move |conn| {
        diesel::delete(news::table.find(id))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if rows == 0 {
        return Err(AppError::not_found("资讯不存在"));
    }
    Ok(ok_msg("资讯已删除", Value::Null))
}
