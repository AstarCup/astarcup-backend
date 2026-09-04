// 玩家对战路由（合并原 player-matchups、player-matchups/delete）
//
//   GET    /api/matchups            列表（管理员看全部；玩家看自己相关，含房间信息）
//   POST   /api/matchups            创建（管理员）
//   PATCH  /api/matchups/{id}       更新状态（管理员）
//   DELETE /api/matchups/{id}       删除（管理员）
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::run_db;
use crate::enums::MatchupStatus;
use crate::error::{ApiResult, AppError, ok_msg};
use crate::models::{MatchRoom, PlayerMatchup};
use crate::schema::{match_rooms, player_matchups};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/matchups", get(list_matchups).post(create_matchup))
        .route(
            "/api/matchups/{id}",
            axum::routing::patch(update_matchup).delete(delete_matchup),
        )
}

// ============ GET /api/matchups ============

async fn list_matchups(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
) -> ApiResult<Json<Value>> {
    let permissions = crate::routes::users_routes::permissions_json(&state, &user.osu_id).await?;
    let is_admin = permissions["isadmin"].as_bool().unwrap_or(false);

    let (matchups, rooms) = run_db(&state.pool, move |conn| {
        let matchups = if is_admin {
            player_matchups::table
                .order(player_matchups::created_at.desc())
                .load::<PlayerMatchup>(conn)
                .map_err(AppError::from)?
        } else {
            player_matchups::table
                .filter(
                    player_matchups::player1_osu_id
                        .eq(&user.osu_id)
                        .or(player_matchups::player2_osu_id.eq(&user.osu_id)),
                )
                .order(player_matchups::created_at.desc())
                .load::<PlayerMatchup>(conn)
                .map_err(AppError::from)?
        };
        let rooms = match_rooms::table
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)?;
        Ok((matchups, rooms))
    })
    .await?;
    let room_by_id: std::collections::HashMap<i32, Value> =
        rooms.into_iter().map(|r| (r.id, r.to_json())).collect();

    let matchups_json: Vec<Value> = matchups
        .iter()
        .map(|m| {
            let room = m.room_id.and_then(|rid| room_by_id.get(&rid));
            m.to_json(room)
        })
        .collect();
    Ok(crate::error::ok(json!(matchups_json)))
}

// ============ POST /api/matchups ============

#[derive(serde::Deserialize)]
struct CreateMatchupBody {
    #[serde(rename = "player1OsuId", alias = "player1_osuId")]
    player1_osu_id: String,
    #[serde(rename = "player1Username", alias = "player1_username")]
    player1_username: String,
    #[serde(rename = "player2OsuId", alias = "player2_osuId")]
    player2_osu_id: String,
    #[serde(rename = "player2Username", alias = "player2_username")]
    player2_username: String,
}

async fn create_matchup(
    State(state): State<AppState>,
    admin: crate::auth::AdminUser,
    Json(body): Json<CreateMatchupBody>,
) -> ApiResult<Json<Value>> {
    if body.player1_osu_id.is_empty()
        || body.player1_username.is_empty()
        || body.player2_osu_id.is_empty()
        || body.player2_username.is_empty()
    {
        return Err(AppError::bad_request("缺少必要字段"));
    }
    let matchup = run_db(&state.pool, move |conn| {
        let now = Utc::now().naive_utc();
        diesel::insert_into(player_matchups::table)
            .values((
                player_matchups::player1_osu_id.eq(&body.player1_osu_id),
                player_matchups::player1_username.eq(&body.player1_username),
                player_matchups::player2_osu_id.eq(&body.player2_osu_id),
                player_matchups::player2_username.eq(&body.player2_username),
                player_matchups::status.eq(MatchupStatus::Available),
                player_matchups::created_by.eq(&admin.osu_id),
                player_matchups::created_at.eq(now),
                player_matchups::updated_at.eq(now),
            ))
            .get_result::<PlayerMatchup>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok_msg("玩家对战创建成功", matchup.to_json(None)))
}

// ============ PATCH /api/matchups/{id} ============

#[derive(serde::Deserialize)]
struct UpdateMatchupBody {
    status: String,
}

async fn update_matchup(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdateMatchupBody>,
) -> ApiResult<Json<Value>> {
    let status =
        MatchupStatus::parse(&body.status).ok_or_else(|| AppError::bad_request("无效的状态"))?;
    let n = run_db(&state.pool, move |conn| {
        diesel::update(player_matchups::table.find(id))
            .set((
                player_matchups::status.eq(status),
                player_matchups::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("对战不存在"));
    }
    Ok(ok_msg("状态更新成功", Value::Null))
}

// ============ DELETE /api/matchups/{id} ============

async fn delete_matchup(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let n = run_db(&state.pool, move |conn| {
        diesel::delete(player_matchups::table.find(id))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("对战不存在"));
    }
    Ok(ok_msg("删除成功", Value::Null))
}
