// 赛季规则（season_rules）管理路由：原 ascBackend/rules/{season}.md 迁入数据库
// Bot 侧读取顺序见 bot_routes::bot_rules：tournament_settings.rules 全局覆盖
// → season_rules 表 → rules/{season}.md 文件兜底
//
//   GET    /api/rules                 全部赛季规则（公开，前台 /guide 使用）
//   GET    /api/admin/rules           全部赛季规则（管理员）
//   PUT    /api/admin/rules/{season}  新建/更新（管理员）
//   DELETE /api/admin/rules/{season}  删除（管理员，删后 Bot 回退读文件）
use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::Value;

use crate::auth::AdminUser;
use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::SeasonRule;
use crate::routes::bot_routes::valid_season_id;
use crate::schema::season_rules;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/rules", get(public_list))
        .route("/api/admin/rules", get(admin_list))
        .route(
            "/api/admin/rules/{season}",
            put(admin_upsert).delete(admin_delete),
        )
}

#[derive(serde::Deserialize)]
struct RuleInput {
    content: String,
}

// ============ GET /api/rules（公开） ============

/// 前台比赛手册 /guide 读取用：与管理后台编辑的是同一份数据
async fn public_list(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let items = run_db(&state.pool, |conn| {
        season_rules::table
            .order(season_rules::season.asc())
            .load::<SeasonRule>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let list: Vec<Value> = items.iter().map(|r| r.to_json()).collect();
    Ok(ok(Value::Array(list)))
}

// ============ GET /api/admin/rules ============

async fn admin_list(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Value>> {
    let items = run_db(&state.pool, |conn| {
        season_rules::table
            .order(season_rules::season.asc())
            .load::<SeasonRule>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let list: Vec<Value> = items.iter().map(|r| r.to_json()).collect();
    Ok(ok(Value::Array(list)))
}

// ============ PUT /api/admin/rules/{season} ============

async fn admin_upsert(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(season): Path<String>,
    Json(input): Json<RuleInput>,
) -> ApiResult<Json<Value>> {
    let season = season.trim().to_lowercase();
    if !valid_season_id(&season) {
        return Err(AppError::bad_request(
            "赛季 ID 只能包含字母、数字、中划线或下划线（最长 32 位）",
        ));
    }
    if input.content.trim().is_empty() {
        return Err(AppError::bad_request("规则内容不能为空"));
    }

    let now = Utc::now().naive_utc();
    let saved = run_db(&state.pool, move |conn| {
        let existing: Option<String> = season_rules::table
            .filter(season_rules::season.eq(&season))
            .select(season_rules::season)
            .first::<String>(conn)
            .optional()
            .map_err(AppError::from)?;
        if existing.is_some() {
            diesel::update(season_rules::table.filter(season_rules::season.eq(&season)))
                .set((
                    season_rules::content.eq(&input.content),
                    season_rules::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(AppError::from)?;
        } else {
            diesel::insert_into(season_rules::table)
                .values((
                    season_rules::season.eq(&season),
                    season_rules::content.eq(&input.content),
                    season_rules::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(AppError::from)?;
        }
        season_rules::table
            .filter(season_rules::season.eq(&season))
            .first::<SeasonRule>(conn)
            .map_err(AppError::from)
    })
    .await?;

    Ok(ok_msg("规则已保存", saved.to_json()))
}

// ============ DELETE /api/admin/rules/{season} ============

async fn admin_delete(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(season): Path<String>,
) -> ApiResult<Json<Value>> {
    let season = season.trim().to_lowercase();
    if !valid_season_id(&season) {
        return Err(AppError::bad_request(
            "赛季 ID 只能包含字母、数字、中划线或下划线（最长 32 位）",
        ));
    }
    let rows = run_db(&state.pool, move |conn| {
        diesel::delete(season_rules::table.filter(season_rules::season.eq(season)))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if rows == 0 {
        return Err(AppError::not_found("该赛季没有规则记录"));
    }
    Ok(ok_msg("规则已删除", Value::Null))
}
