// 首页画布贴纸（/ 的 1600×900 大画布）
//
//   GET    /api/canvas-stickers?page=home   公开列表
//   POST   /api/canvas-stickers             贴一个（登录 + 限速）
//   PATCH  /api/canvas-stickers/{id}        移动 / 缩放 / 旋转（本人或管理员）
//   DELETE /api/canvas-stickers/{id}        删除（本人或管理员）
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, patch};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::auth::{AdminUser, SessionUser};
use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::CanvasSticker;
use crate::schema::canvas_stickers;
use crate::state::AppState;

/// 单个画布最多多少张
const MAX_PER_PAGE: i64 = 80;
/// 每分钟最多贴多少张
const MAX_PER_MINUTE: i64 = 20;
/// 列表上限
const MAX_LIST: i64 = 500;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/canvas-stickers", get(list).post(create))
        .route(
            "/api/canvas-stickers/{id}",
            patch(update).delete(remove),
        )
}

#[derive(serde::Deserialize)]
struct ListQuery {
    #[serde(default)]
    page: Option<String>,
}

fn valid_token(token: &str) -> bool {
    !token.is_empty()
        && token.chars().count() <= 42
        && token
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn clamp_rotation(r: f64) -> f64 {
    // 归一化到 -180..180，避免存出奇怪的角度
    let mut v = r % 360.0;
    if v > 180.0 {
        v -= 360.0;
    }
    if v < -180.0 {
        v += 360.0;
    }
    v
}

// ============ GET ============

async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    let page = q.page.unwrap_or_else(|| "home".to_string());
    let items = run_db(&state.pool, move |conn| {
        canvas_stickers::table
            .filter(canvas_stickers::page.eq(&page))
            .order(canvas_stickers::id.asc())
            .limit(MAX_LIST)
            .load::<CanvasSticker>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let list: Vec<Value> = items.iter().map(|s| s.to_json()).collect();
    Ok(ok(Value::Array(list)))
}

// ============ POST ============

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    #[serde(default)]
    page: Option<String>,
    token: String,
    x: f64,
    y: f64,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default)]
    rotation: f64,
}

fn default_scale() -> f64 {
    1.0
}

async fn create(
    State(state): State<AppState>,
    user: SessionUser,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<Value>> {
    if !valid_token(&body.token) {
        return Err(AppError::bad_request("表情 token 不合法"));
    }
    if !body.x.is_finite() || !body.y.is_finite() || body.x.abs() > 10000.0 || body.y.abs() > 10000.0 {
        return Err(AppError::bad_request("坐标不合法"));
    }
    let page = body.page.unwrap_or_else(|| "home".to_string());
    if page.len() > 32 {
        return Err(AppError::bad_request("page 过长"));
    }
    let x = body.x.clamp(-10000.0, 10000.0);
    let y = body.y.clamp(-10000.0, 10000.0);
    let scale = body.scale.clamp(0.2, 4.0);
    let rotation = clamp_rotation(body.rotation);
    let token = body.token.clone();
    let created_by = user.osu_id.clone();
    let cutoff = Utc::now().naive_utc() - Duration::minutes(1);

    let saved = run_db(&state.pool, move |conn| {
        let recent = canvas_stickers::table
            .filter(canvas_stickers::created_by.eq(&created_by))
            .filter(canvas_stickers::created_at.gt(cutoff))
            .count()
            .get_result::<i64>(conn)
            .map_err(AppError::from)?;
        if recent >= MAX_PER_MINUTE {
            return Err(AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "贴得太快啦，歇一会儿（每分钟最多 20 个）",
            ));
        }

        let on_page = canvas_stickers::table
            .filter(canvas_stickers::page.eq(&page))
            .count()
            .get_result::<i64>(conn)
            .map_err(AppError::from)?;
        if on_page >= MAX_PER_PAGE {
            return Err(AppError::bad_request(format!(
                "画布上最多贴 {MAX_PER_PAGE} 个表情"
            )));
        }

        diesel::insert_into(canvas_stickers::table)
            .values((
                canvas_stickers::page.eq(&page),
                canvas_stickers::token.eq(&token),
                canvas_stickers::x.eq(x),
                canvas_stickers::y.eq(y),
                canvas_stickers::scale.eq(scale),
                canvas_stickers::rotation.eq(rotation),
                canvas_stickers::created_by.eq(&created_by),
            ))
            .get_result::<CanvasSticker>(conn)
            .map_err(AppError::from)
    })
    .await?;

    Ok(ok(saved.to_json()))
}

// ============ PATCH（移动 / 缩放 / 旋转） ============

#[derive(serde::Deserialize)]
struct UpdateBody {
    x: f64,
    y: f64,
    scale: f64,
    rotation: f64,
}

async fn update(
    State(state): State<AppState>,
    user: SessionUser,
    admin: Option<AdminUser>,
    Path(id): Path<i32>,
    Json(body): Json<UpdateBody>,
) -> ApiResult<Json<Value>> {
    if !body.x.is_finite() || !body.y.is_finite() || body.x.abs() > 10000.0 || body.y.abs() > 10000.0 {
        return Err(AppError::bad_request("坐标不合法"));
    }
    let x = body.x.clamp(-10000.0, 10000.0);
    let y = body.y.clamp(-10000.0, 10000.0);
    let scale = body.scale.clamp(0.2, 4.0);
    let rotation = clamp_rotation(body.rotation);
    let is_admin = admin.is_some();
    let osu_id = user.osu_id.clone();

    let saved = run_db(&state.pool, move |conn| {
        let sticker = canvas_stickers::table
            .filter(canvas_stickers::id.eq(id))
            .first::<CanvasSticker>(conn)
            .optional()
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("贴纸不存在"))?;
        if !is_admin && sticker.created_by != osu_id {
            return Err(AppError::forbidden("只能调整自己贴的表情"));
        }
        diesel::update(canvas_stickers::table.filter(canvas_stickers::id.eq(id)))
            .set((
                canvas_stickers::x.eq(x),
                canvas_stickers::y.eq(y),
                canvas_stickers::scale.eq(scale),
                canvas_stickers::rotation.eq(rotation),
            ))
            .execute(conn)
            .map_err(AppError::from)?;
        canvas_stickers::table
            .filter(canvas_stickers::id.eq(id))
            .first::<CanvasSticker>(conn)
            .map_err(AppError::from)
    })
    .await?;

    Ok(ok(saved.to_json()))
}

// ============ DELETE ============

async fn remove(
    State(state): State<AppState>,
    user: SessionUser,
    admin: Option<AdminUser>,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let is_admin = admin.is_some();
    let osu_id = user.osu_id.clone();
    run_db(&state.pool, move |conn| {
        let sticker = canvas_stickers::table
            .filter(canvas_stickers::id.eq(id))
            .first::<CanvasSticker>(conn)
            .optional()
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("贴纸不存在"))?;
        if !is_admin && sticker.created_by != osu_id {
            return Err(AppError::forbidden("只能删除自己贴的表情"));
        }
        diesel::delete(canvas_stickers::table.filter(canvas_stickers::id.eq(id)))
            .execute(conn)
            .map_err(AppError::from)?;
        Ok(())
    })
    .await?;
    Ok(ok_msg("贴纸已删除", json!(null)))
}
