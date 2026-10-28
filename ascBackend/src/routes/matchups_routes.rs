// 玩家对战路由（合并原 player-matchups、player-matchups/delete）
//
//   GET    /api/matchups?season=     列表（管理员看全部；玩家看自己相关，含房间信息）
//   POST   /api/matchups            创建（管理员，默认落当前赛季）
//   PATCH  /api/matchups/{id}       更新状态 / 赛季（管理员）
//   DELETE /api/matchups/{id}       删除（管理员）
use axum::extract::{Path, Query, State};
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

#[derive(serde::Deserialize)]
struct MatchupsQuery {
    /// 赛季过滤：不传 = 全部
    season: Option<String>,
}

async fn list_matchups(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Query(q): Query<MatchupsQuery>,
) -> ApiResult<Json<Value>> {
    let permissions = crate::routes::users_routes::permissions_json(&state, &user.osu_id).await?;
    let is_admin = permissions["isadmin"].as_bool().unwrap_or(false);
    let season_filter = q
        .season
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty() && s != "all");

    let (matchups, rooms) = run_db(&state.pool, move |conn| {
        let mut query = if is_admin {
            player_matchups::table.into_boxed()
        } else {
            player_matchups::table
                .filter(
                    player_matchups::player1_osu_id
                        .eq(&user.osu_id)
                        .or(player_matchups::player2_osu_id.eq(&user.osu_id)),
                )
                .into_boxed()
        };
        if let Some(s) = season_filter {
            query = query.filter(player_matchups::season.eq(s));
        }
        let matchups = query
            .order(player_matchups::created_at.desc())
            .load::<PlayerMatchup>(conn)
            .map_err(AppError::from)?;
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
    #[serde(rename = "player2OsuId", alias = "player2_osuId")]
    player2_osu_id: String,
    #[serde(rename = "player1Username", alias = "player1_username")]
    player1_username: String,
    #[serde(rename = "player2Username", alias = "player2_username")]
    player2_username: String,
    /// 赛季 ID；不传时取系统设置里的 current_season
    season: Option<String>,
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
    // 赛季：显式传入优先，否则落当前赛季
    let season = match body.season.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_ascii_lowercase(),
        _ => crate::routes::schedules_routes::current_season(&state).await?,
    };
    let matchup = run_db(&state.pool, move |conn| {
        let now = Utc::now().naive_utc();
        diesel::insert_into(player_matchups::table)
            .values((
                player_matchups::player1_osu_id.eq(&body.player1_osu_id),
                player_matchups::player1_username.eq(&body.player1_username),
                player_matchups::player2_osu_id.eq(&body.player2_osu_id),
                player_matchups::player2_username.eq(&body.player2_username),
                player_matchups::status.eq(MatchupStatus::Available),
                player_matchups::season.eq(season),
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
    status: Option<String>,
    /// 把对战改到另一个赛季
    season: Option<String>,
}

async fn update_matchup(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdateMatchupBody>,
) -> ApiResult<Json<Value>> {
    let status = match body.status.as_deref() {
        Some(s) => {
            Some(MatchupStatus::parse(s).ok_or_else(|| AppError::bad_request("无效的状态"))?)
        }
        None => None,
    };
    let season = body
        .season
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase);
    if status.is_none() && season.is_none() {
        return Err(AppError::bad_request("没有要更新的字段"));
    }
    let n = run_db(&state.pool, move |conn| {
        conn.transaction(|txn| {
            let now = Utc::now().naive_utc();
            let target = player_matchups::table.find(id);
            let updated = match (status, season) {
                (Some(st), Some(se)) => diesel::update(target)
                    .set((
                        player_matchups::status.eq(st),
                        player_matchups::season.eq(se),
                        player_matchups::updated_at.eq(now),
                    ))
                    .execute(txn),
                (Some(st), None) => diesel::update(target)
                    .set((
                        player_matchups::status.eq(st),
                        player_matchups::updated_at.eq(now),
                    ))
                    .execute(txn),
                (None, Some(se)) => diesel::update(target)
                    .set((
                        player_matchups::season.eq(se),
                        player_matchups::updated_at.eq(now),
                    ))
                    .execute(txn),
                (None, None) => Ok(0),
            }
            .map_err(AppError::from)?;
            Ok(updated)
        })
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("对战不存在"));
    }
    Ok(ok_msg("对战已更新", Value::Null))
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
