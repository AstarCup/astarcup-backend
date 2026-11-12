// 选手卡片表情贴纸（/registrations 页面）
//
//   GET    /api/stickers        全部贴纸（公开，前台一次拉完）
//   POST   /api/stickers        贴一个表情（需登录）{ osuId, token, x, y }
//   DELETE /api/stickers/{id}   删除贴纸（本人或管理员）
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use serde_json::Value;

use crate::auth::{AdminUser, SessionUser};
use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::PlayerSticker;
use crate::schema::{player_stickers, users};
use crate::state::AppState;

/// 单个选手卡片最多贴多少个
const MAX_PER_PLAYER: i64 = 12;
/// 单个用户累计最多贴多少个（防刷）
const MAX_PER_USER: i64 = 200;
/// 单个用户每分钟最多贴多少个
const MAX_PER_MINUTE: i64 = 20;
/// 一次返回的上限
const MAX_LIST: i64 = 2000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/stickers", get(list).post(create))
        .route("/api/stickers/{id}", delete(remove))
}

// ============ GET /api/stickers ============

async fn list(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let items = run_db(&state.pool, |conn| {
        player_stickers::table
            .order(player_stickers::id.asc())
            .limit(MAX_LIST)
            .load::<PlayerSticker>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let list: Vec<Value> = items.iter().map(|s| s.to_json()).collect();
    Ok(ok(Value::Array(list)))
}

// ============ POST /api/stickers ============

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    osu_id: String,
    token: String,
    x: f64,
    y: f64,
}

fn valid_token(token: &str) -> bool {
    !token.is_empty()
        && token.chars().count() <= 42
        && token
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

async fn create(
    State(state): State<AppState>,
    user: SessionUser,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<Value>> {
    let osu_id = body.osu_id.trim().to_string();
    if osu_id.is_empty() {
        return Err(AppError::bad_request("缺少 osuId"));
    }
    if !valid_token(&body.token) {
        return Err(AppError::bad_request("表情 token 不合法"));
    }
    if !body.x.is_finite() || !body.y.is_finite() {
        return Err(AppError::bad_request("坐标不合法"));
    }
    let x = body.x.clamp(0.0, 1.0);
    let y = body.y.clamp(0.0, 1.0);
    let token = body.token.clone();
    let created_by = user.osu_id.clone();
    let cutoff = Utc::now().naive_utc() - Duration::minutes(1);

    let saved = run_db(&state.pool, move |conn| {
        // 贴纸只能贴到真实存在的选手上
        let exists = users::table
            .filter(users::osu_id.eq(&osu_id))
            .count()
            .get_result::<i64>(conn)
            .map_err(AppError::from)?;
        if exists == 0 {
            return Err(AppError::not_found("该选手不存在"));
        }

        // 限速：每分钟最多 MAX_PER_MINUTE 个
        let recent = player_stickers::table
            .filter(player_stickers::created_by.eq(&created_by))
            .filter(player_stickers::created_at.gt(cutoff))
            .count()
            .get_result::<i64>(conn)
            .map_err(AppError::from)?;
        if recent >= MAX_PER_MINUTE {
            return Err(AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "贴得太快啦，歇一会儿（每分钟最多 20 个）",
            ));
        }

        let on_player = player_stickers::table
            .filter(player_stickers::osu_id.eq(&osu_id))
            .count()
            .get_result::<i64>(conn)
            .map_err(AppError::from)?;
        if on_player >= MAX_PER_PLAYER {
            return Err(AppError::bad_request(format!(
                "每个选手最多贴 {MAX_PER_PLAYER} 个表情"
            )));
        }

        let by_user = player_stickers::table
            .filter(player_stickers::created_by.eq(&created_by))
            .count()
            .get_result::<i64>(conn)
            .map_err(AppError::from)?;
        if by_user >= MAX_PER_USER {
            return Err(AppError::bad_request("你贴的表情已经很多啦"));
        }

        diesel::insert_into(player_stickers::table)
            .values((
                player_stickers::osu_id.eq(&osu_id),
                player_stickers::token.eq(&token),
                player_stickers::x.eq(x),
                player_stickers::y.eq(y),
                player_stickers::created_by.eq(&created_by),
            ))
            .get_result::<PlayerSticker>(conn)
            .map_err(AppError::from)
    })
    .await?;

    Ok(ok(saved.to_json()))
}

// ============ DELETE /api/stickers/{id} ============

async fn remove(
    State(state): State<AppState>,
    user: SessionUser,
    admin: Option<AdminUser>,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let is_admin = admin.is_some();
    let osu_id = user.osu_id.clone();
    run_db(&state.pool, move |conn| {
        let sticker = player_stickers::table
            .filter(player_stickers::id.eq(id))
            .first::<PlayerSticker>(conn)
            .optional()
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("贴纸不存在"))?;
        if !is_admin && sticker.created_by != osu_id {
            return Err(AppError::forbidden("只能删除自己贴的表情"));
        }
        diesel::delete(player_stickers::table.filter(player_stickers::id.eq(id)))
            .execute(conn)
            .map_err(AppError::from)?;
        Ok(())
    })
    .await?;

    Ok(ok_msg("贴纸已删除", Value::Null))
}
