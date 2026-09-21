// 用户/报名路由（合并原 users、user-registration、check-registration、edge-registrations、
// register、approved-players、obs-overlay/players、get-user-info、player-next-match、admin/* 等）
//
//   GET    /api/users                     报名列表（registrationStatus ∈ registered/approved）
//   POST   /api/users                     报名（赛季逗号追加去重）
//   GET    /api/users/{osu_id}            用户详情 + 报名状态
//   GET    /api/players?status=           玩家列表（status=approved 需管理员；默认全部，公开）
//   GET    /api/me/permissions            当前用户权限 {isplayer, isadmin}
//   GET    /api/me/next-match             当前玩家下一场可用对战
//   POST   /api/osu/users/{user_id}       osu! API 用户信息代理
//   GET    /api/admin/users               全部用户（管理员）
//   POST   /api/admin/users/{osu_id}/approve   审核通过（管理员）
//   PATCH  /api/admin/users/{osu_id}      修改用户组（管理员）
//   DELETE /api/admin/users/{osu_id}      删除用户（管理员）
use axum::extract::{Path, Query, State};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::run_db;
use crate::enums::{RegistrationStatus, UserGroup};
use crate::error::{ApiResult, AppError, ok};
use crate::models::User;
use crate::schema::users;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/users", get(list_registrations).post(register))
        .route("/api/users/{osu_id}", get(user_detail))
        .route("/api/players", get(list_players))
        .route("/api/me/permissions", get(me_permissions))
        .route("/api/me/next-match", get(me_next_match))
        .route("/api/osu/users/{user_id}", post(osu_user_proxy))
        .route("/api/admin/users", get(admin_list_users))
        .route(
            "/api/admin/users/{osu_id}/approve",
            post(admin_approve_user),
        )
        .route(
            "/api/admin/users/{osu_id}",
            patch(admin_update_user).delete(admin_delete_user),
        )
}

// ============ GET /api/users ============

#[derive(serde::Deserialize)]
struct UsersQuery {
    status: Option<String>,
}

async fn list_registrations(
    State(state): State<AppState>,
    Query(q): Query<UsersQuery>,
) -> ApiResult<Json<Value>> {
    let want_all = q.status.as_deref() == Some("all");
    let list = run_db(&state.pool, move |conn| {
        let mut query = users::table.into_boxed();
        if !want_all {
            query = query.filter(
                users::registration_status
                    .eq_any([RegistrationStatus::Registered, RegistrationStatus::Approved]),
            );
        }
        query
            .order(users::registered_at.desc())
            .load::<User>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok(json!(
        list.iter().map(|u| u.to_json()).collect::<Vec<_>>()
    )))
}

// ============ POST /api/users（报名） ============

#[derive(serde::Deserialize)]
struct RegisterBody {
    #[serde(rename = "osuId")]
    osu_id: String,
    username: String,
    country: String,
    #[serde(rename = "avatar_url", default)]
    avatar_url: Option<String>,
    #[serde(default)]
    pp: Option<f64>,
    #[serde(rename = "global_rank", default)]
    global_rank: Option<i32>,
    #[serde(rename = "country_rank", default)]
    country_rank: Option<i32>,
    #[serde(default)]
    accuracy: Option<f64>,
    #[serde(default)]
    stamina: Option<f64>,
    #[serde(rename = "firstSight", default)]
    first_sight: Option<f64>,
    #[serde(default)]
    strategy: Option<f64>,
    #[serde(default)]
    experience: Option<f64>,
    #[serde(rename = "customKey", default)]
    custom_key: Option<String>,
    #[serde(rename = "customValue", default)]
    custom_value: Option<f64>,
    #[serde(rename = "cover_custom_url", default)]
    cover_custom_url: Option<String>,
    #[serde(rename = "cover_url", default)]
    cover_url: Option<String>,
    #[serde(rename = "cover_id", default)]
    cover_id: Option<String>,
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterBody>,
) -> ApiResult<Json<Value>> {
    if body.osu_id.is_empty() || body.username.is_empty() || body.country.is_empty() {
        return Err(AppError::bad_request(
            "缺少必要字段 (osuId, username, country)",
        ));
    }

    // 报名门槛：必须先绑定 g0v0 服务器账号
    let osu_id_check = body.osu_id.clone();
    let bound: Option<String> = run_db(&state.pool, move |conn| {
        users::table
            .filter(users::osu_id.eq(&osu_id_check))
            .select(users::gu_server_user_id)
            .first::<Option<String>>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?
    .flatten();
    if bound.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
        return Err(AppError::forbidden("请先绑定 g0v0 服务器账号后再报名"));
    }

    let season = current_season(&state).await?;

    // 赛季逗号追加去重 + 报名
    let osu_id = body.osu_id.clone();
    let season_c = season.clone();
    let user = run_db(&state.pool, move |conn| {
        let now = Utc::now().naive_utc();
        let existing: Option<User> = users::table
            .filter(users::osu_id.eq(&osu_id))
            .first(conn)
            .optional()
            .map_err(AppError::from)?;
        if let Some(u) = existing {
            let mut seasons: Vec<String> = u.season.clone().unwrap_or_default();
            if !seasons.contains(&season_c) {
                seasons.push(season_c.clone());
            }
            diesel::update(users::table.find(u.id))
                .set((
                    users::season.eq(Some(seasons)),
                    users::registration_status.eq(RegistrationStatus::Registered),
                    users::accuracy.eq(body.accuracy),
                    users::stamina.eq(body.stamina),
                    users::first_sight.eq(body.first_sight),
                    users::strategy.eq(body.strategy),
                    users::experience.eq(body.experience),
                    users::custom_key.eq(body.custom_key),
                    users::custom_value.eq(body.custom_value),
                    users::cover_custom_url.eq(body.cover_custom_url),
                    users::cover_url.eq(body.cover_url),
                    users::cover_id.eq(body.cover_id),
                    users::updated_at.eq(now),
                ))
                .get_result::<User>(conn)
                .map_err(AppError::from)
        } else {
            diesel::insert_into(users::table)
                .values((
                    users::osu_id.eq(&body.osu_id),
                    users::username.eq(&body.username),
                    users::country.eq(&body.country),
                    users::registered_at.eq(now),
                    users::avatar_url.eq(&body.avatar_url),
                    users::pp.eq(body.pp),
                    users::global_rank.eq(body.global_rank),
                    users::country_rank.eq(body.country_rank),
                    users::user_group.eq(UserGroup::Player),
                    users::registration_status.eq(RegistrationStatus::Registered),
                    users::season.eq(Some(vec![season_c.clone()])),
                    users::accuracy.eq(body.accuracy),
                    users::stamina.eq(body.stamina),
                    users::first_sight.eq(body.first_sight),
                    users::strategy.eq(body.strategy),
                    users::experience.eq(body.experience),
                    users::custom_key.eq(body.custom_key),
                    users::custom_value.eq(body.custom_value),
                    users::cover_custom_url.eq(&body.cover_custom_url),
                    users::cover_url.eq(&body.cover_url),
                    users::cover_id.eq(&body.cover_id),
                    users::created_at.eq(now),
                    users::updated_at.eq(now),
                ))
                .get_result::<User>(conn)
                .map_err(AppError::from)
        }
    })
    .await?;
    Ok(crate::error::ok_msg("报名成功", user.to_json()))
}

/// 读取 TournamentSetting.current_season（默认 s1）
pub async fn current_season(state: &AppState) -> Result<String, AppError> {
    run_db(&state.pool, |conn| {
        use crate::schema::tournament_settings;
        let v: Option<String> = tournament_settings::table
            .filter(tournament_settings::setting_key.eq("current_season"))
            .select(tournament_settings::setting_value)
            .first(conn)
            .optional()
            .map_err(AppError::from)?;
        Ok(v.unwrap_or_else(|| "s1".into())
            .trim_matches(|c| c == '"' || c == '\'')
            .trim()
            .to_string())
    })
    .await
}

// ============ GET /api/users/{osu_id} ============

async fn user_detail(
    State(state): State<AppState>,
    Path(osu_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let user = run_db(&state.pool, move |conn| {
        users::table
            .filter(users::osu_id.eq(&osu_id))
            .first::<User>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;
    match user {
        Some(u) => Ok(ok(u.to_json())),
        None => Err(AppError::not_found("用户不存在")),
    }
}

// ============ GET /api/players?status= ============

#[derive(serde::Deserialize)]
struct PlayersQuery {
    status: Option<String>,
}

async fn list_players(
    state: State<AppState>,
    Query(q): Query<PlayersQuery>,
    admin: Option<crate::auth::AdminUser>,
) -> ApiResult<Json<Value>> {
    let want_approved = q.status.as_deref() == Some("approved");
    if want_approved && admin.is_none() {
        return Err(AppError::forbidden("权限不足"));
    }
    let list = run_db(&state.pool, move |conn| {
        let mut query = users::table.into_boxed();
        if want_approved {
            query = query.filter(users::registration_status.eq(RegistrationStatus::Approved));
        }
        query.load::<User>(conn).map_err(AppError::from)
    })
    .await?;
    let players: Vec<Value> = list
        .iter()
        .map(|u| {
            json!({
                "osuId": u.osu_id,
                "username": u.username,
                "avatarUrl": u.avatar_url,
                "pp": u.pp,
                "globalRank": u.global_rank,
                "countryRank": u.country_rank,
                "country": u.country,
                "registrationStatus": u.registration_status.as_str(),
            })
        })
        .collect();
    Ok(ok(json!(players)))
}

// ============ GET /api/me/permissions ============

async fn me_permissions(
    State(state): State<AppState>,
    user: Option<crate::auth::SessionUser>,
) -> ApiResult<Json<Value>> {
    let Some(user) = user else {
        return Ok(ok(json!({ "isplayer": false, "isadmin": false })));
    };
    let permissions = permissions_json(&state, &user.osu_id).await?;
    Ok(ok(permissions))
}

/// 权限判定：isadmin = userGroup==admin；isplayer = 非管理员且已报名
pub async fn permissions_json(state: &AppState, osu_id: &str) -> Result<Value, AppError> {
    let osu_id = osu_id.to_string();
    let u = run_db(&state.pool, move |conn| {
        users::table
            .filter(users::osu_id.eq(&osu_id))
            .select((users::user_group, users::registration_status))
            .first::<(UserGroup, RegistrationStatus)>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;
    let is_admin = u
        .as_ref()
        .map(|(g, _)| *g == UserGroup::Admin)
        .unwrap_or(false);
    let is_registered = u
        .as_ref()
        .map(|(_, s)| {
            matches!(
                s,
                RegistrationStatus::Registered | RegistrationStatus::Approved
            )
        })
        .unwrap_or(false);
    Ok(json!({
        "isplayer": !is_admin && is_registered,
        "isadmin": is_admin,
    }))
}

// ============ GET /api/me/next-match ============

async fn me_next_match(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
) -> ApiResult<Json<Value>> {
    use crate::schema::{match_rooms, player_matchups};
    let osu_id = user.osu_id.clone();
    let matchup = run_db(&state.pool, move |conn| {
        player_matchups::table
            .filter(
                player_matchups::player1_osu_id
                    .eq(&osu_id)
                    .or(player_matchups::player2_osu_id.eq(&osu_id)),
            )
            .filter(
                player_matchups::status.ne(crate::enums::MatchupStatus::Completed),
            )
            .order(player_matchups::created_at.asc())
            .first::<crate::models::PlayerMatchup>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;

    let Some(m) = matchup else {
        return Ok(ok(Value::Null));
    };
    let opponent_osu_id = if m.player1_osu_id == user.osu_id {
        m.player2_osu_id.clone()
    } else {
        m.player1_osu_id.clone()
    };
    let fallback_username = if m.player1_osu_id == user.osu_id {
        m.player2_username.clone()
    } else {
        m.player1_username.clone()
    };
    let opponent_lookup_id = opponent_osu_id.clone();
    let opponent = run_db(&state.pool, move |conn| {
        users::table
            .filter(users::osu_id.eq(&opponent_lookup_id))
            .select((users::username, users::avatar_url))
            .first::<(String, Option<String>)>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;
    let (username, avatar) = match opponent {
        Some((name, avatar)) => (name, avatar),
        None => (fallback_username, None),
    };

    // 附带预约/已安排的房间信息（含比赛时间），供玩家查看
    let mut room_json = Value::Null;
    if let Some(room_id) = m.room_id {
        let room = run_db(&state.pool, move |conn| {
            match_rooms::table
                .find(room_id)
                .first::<crate::models::MatchRoom>(conn)
                .optional()
                .map_err(AppError::from)
        })
        .await?;
        if let Some(r) = room {
            room_json = r.to_json();
        }
    }

    Ok(ok(json!({
        "id": m.id,
        "opponent": { "osuId": opponent_osu_id, "username": username, "avatarUrl": avatar },
        "status": m.status.as_str(),
        "room": room_json,
    })))
}

// ============ POST /api/osu/users/{user_id} ============

async fn osu_user_proxy(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> ApiResult<Json<Value>> {
    if user_id.parse::<i64>().is_err() {
        return Err(AppError::bad_request("无效的用户ID"));
    }
    let me = state
        .osu
        .get_json(&format!("/users/{user_id}"), &[])
        .await?;
    Ok(ok(json!({
        "user": {
            "id": me["id"],
            "username": me["username"],
            "avatarUrl": me["avatar_url"],
            "countryCode": me["country_code"],
            "pp": me["statistics"]["pp"],
            "globalRank": me["statistics"]["global_rank"],
            "cover": me["cover"],
        }
    })))
}

// ============ 管理员端点 ============

async fn admin_list_users(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
) -> ApiResult<Json<Value>> {
    let list = run_db(&state.pool, move |conn| {
        users::table
            .select((
                users::osu_id,
                users::username,
                users::avatar_url,
                users::user_group,
                users::registration_status,
                users::created_at,
                users::season,
            ))
            .order(users::created_at.desc())
            .load::<(
                String,
                String,
                Option<String>,
                UserGroup,
                RegistrationStatus,
                chrono::NaiveDateTime,
                Option<Vec<String>>,
            )>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let users_json: Vec<Value> = list
        .into_iter()
        .map(|(osu_id, username, avatar, group, reg, created, season)| {
            json!({
                "osuId": osu_id,
                "username": username,
                "avatarUrl": avatar,
                "userGroup": group.as_str(),
                "registrationStatus": reg.as_str(),
                "createdAt": created.and_utc().to_rfc3339(),
                "season": season,
            })
        })
        .collect();
    Ok(ok(json!(users_json)))
}

async fn admin_approve_user(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(osu_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let n = run_db(&state.pool, move |conn| {
        diesel::update(users::table.filter(users::osu_id.eq(&osu_id)))
            .set(users::registration_status.eq(RegistrationStatus::Approved))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("用户不存在"));
    }
    Ok(crate::error::ok_msg("用户审核通过", Value::Null))
}

#[derive(serde::Deserialize)]
struct UpdateUserGroupBody {
    #[serde(rename = "userGroup")]
    user_group: Option<String>,
    #[serde(rename = "registrationStatus")]
    registration_status: Option<String>,
}

async fn admin_update_user(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(osu_id): Path<String>,
    Json(body): Json<UpdateUserGroupBody>,
) -> ApiResult<Json<Value>> {
    let n = run_db(&state.pool, move |conn| {
        if let Some(g) = &body.user_group {
            let group = UserGroup::parse(g)
                .ok_or_else(|| AppError::bad_request("userGroup必须是player或admin"))?;
            diesel::update(users::table.filter(users::osu_id.eq(&osu_id)))
                .set(users::user_group.eq(group))
                .execute(conn)
                .map_err(AppError::from)?;
        }
        if let Some(s) = &body.registration_status {
            let status = RegistrationStatus::parse(s)
                .ok_or_else(|| AppError::bad_request("无效的报名状态"))?;
            diesel::update(users::table.filter(users::osu_id.eq(&osu_id)))
                .set(users::registration_status.eq(status))
                .execute(conn)
                .map_err(AppError::from)?;
        }
        users::table
            .filter(users::osu_id.eq(&osu_id))
            .count()
            .get_result::<i64>(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("用户不存在"));
    }
    Ok(crate::error::ok_msg("用户更新成功", Value::Null))
}

async fn admin_delete_user(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(osu_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let n = run_db(&state.pool, move |conn| {
        diesel::delete(users::table.filter(users::osu_id.eq(&osu_id)))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("删除失败，可能用户不存在"));
    }
    Ok(crate::error::ok_msg("用户注册删除成功", Value::Null))
}
