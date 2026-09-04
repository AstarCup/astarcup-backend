// 比赛预约路由（合并原 match-schedules、create、admin-create、update、update-details）
//
//   GET   /api/schedules                预约列表（含房间）
//   POST  /api/schedules                三模式创建：管理员模式 / 消息通知模式 / 普通模式
//   PATCH /api/schedules/{id}           状态更新（玩家确认/取消权限链）
//   PATCH /api/schedules/{id}/details   详情更新（管理员：比分/链接/状态）
use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::run_db;
use crate::enums::{MatchupStatus, RoomStatus, ScheduleStatus};
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::{MatchRoom, MatchSchedule};
use crate::routes::users_routes::permissions_json;
use crate::schema::{match_rooms, match_schedules, messages, player_matchups};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/schedules", get(list_schedules).post(create_schedule))
        .route("/api/schedules/{id}", patch(update_schedule_status))
        .route(
            "/api/schedules/{id}/details",
            patch(update_schedule_details),
        )
}

// ============ GET /api/schedules ============

async fn list_schedules(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let schedules = run_db(&state.pool, |conn| {
        match_schedules::table
            .order(match_schedules::created_at.desc())
            .load::<MatchSchedule>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let rooms = run_db(&state.pool, |conn| {
        match_rooms::table
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let room_by_id: std::collections::HashMap<i32, Value> =
        rooms.into_iter().map(|r| (r.id, r.to_json())).collect();

    let schedules_json: Vec<Value> = schedules
        .iter()
        .map(|s| {
            let room = room_by_id.get(&s.room_id);
            s.to_json(room)
        })
        .collect();
    Ok(ok(json!(schedules_json)))
}

// ============ POST /api/schedules ============

#[derive(serde::Deserialize)]
struct CreateScheduleBody {
    #[serde(rename = "matchupId", alias = "matchup_id", default)]
    matchup_id: Option<i64>,
    #[serde(rename = "roomId", alias = "room_id", default)]
    room_id: Option<i64>,
    #[serde(rename = "isAdminCreate", alias = "is_admin_create", default)]
    is_admin_create: bool,
    #[serde(rename = "sendNotification", alias = "send_notification", default)]
    send_notification: bool,
    // 管理员模式专用
    #[serde(rename = "player1OsuId", alias = "player1_osuId", default)]
    player1_osu_id: Option<String>,
    #[serde(rename = "player1Username", alias = "player1_username", default)]
    player1_username: Option<String>,
    #[serde(rename = "player2OsuId", alias = "player2_osuId", default)]
    player2_osu_id: Option<String>,
    #[serde(rename = "player2Username", alias = "player2_username", default)]
    player2_username: Option<String>,
}

async fn create_schedule(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Json(body): Json<CreateScheduleBody>,
) -> ApiResult<Json<Value>> {
    let permissions = permissions_json(&state, &user.osu_id).await?;
    let is_admin = permissions["isadmin"].as_bool().unwrap_or(false);

    // 模式 1：管理员创建（不需要对战 ID）
    if body.is_admin_create {
        if !is_admin {
            return Err(AppError::forbidden("需要管理员权限"));
        }
        let room_id = body
            .room_id
            .ok_or_else(|| AppError::bad_request("缺少必填字段：房间ID、选手1信息、选手2信息"))?;
        let p1 = body
            .player1_osu_id
            .ok_or_else(|| AppError::bad_request("缺少选手1信息"))?;
        let p1u = body
            .player1_username
            .ok_or_else(|| AppError::bad_request("缺少选手1信息"))?;
        let p2 = body
            .player2_osu_id
            .ok_or_else(|| AppError::bad_request("缺少选手2信息"))?;
        let p2u = body
            .player2_username
            .ok_or_else(|| AppError::bad_request("缺少选手2信息"))?;
        let room_id = i32::try_from(room_id).map_err(|_| AppError::bad_request("无效的房间ID"))?;
        let schedule =
            insert_schedule(&state, room_id, p1, p1u, p2, p2u, user.osu_id.clone()).await?;
        return Ok(ok_msg(
            "比赛预约创建成功（管理员模式）",
            schedule.to_json(None),
        ));
    }

    // 模式 2/3：基于现有对战创建
    let matchup_id = body
        .matchup_id
        .ok_or_else(|| AppError::bad_request("缺少对战ID或房间ID"))?;
    let room_id = body
        .room_id
        .ok_or_else(|| AppError::bad_request("缺少对战ID或房间ID"))?;
    let matchup_id =
        i32::try_from(matchup_id).map_err(|_| AppError::bad_request("无效的对战ID"))?;
    let room_id = i32::try_from(room_id).map_err(|_| AppError::bad_request("无效的房间ID"))?;

    // 取对战并校验状态与参赛资格
    let matchup = run_db(&state.pool, move |conn| {
        player_matchups::table
            .find(matchup_id)
            .first::<crate::models::PlayerMatchup>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?
    .ok_or_else(|| AppError::not_found("对战信息不存在"))?;
    if matchup.status != MatchupStatus::Available {
        return Err(AppError::bad_request("该对战已被预约或已完成"));
    }
    if !is_admin && user.osu_id != matchup.player1_osu_id && user.osu_id != matchup.player2_osu_id {
        return Err(AppError::forbidden("您不是该对战的参赛选手"));
    }

    // 房间必须存在且 open
    let room = run_db(&state.pool, move |conn| {
        match_rooms::table
            .find(room_id)
            .first::<MatchRoom>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?
    .ok_or_else(|| AppError::not_found("房间不存在"))?;
    if room.status != RoomStatus::Open {
        return Err(AppError::bad_request("房间不可用"));
    }

    let schedule = insert_schedule(
        &state,
        room_id,
        matchup.player1_osu_id.clone(),
        matchup.player1_username.clone(),
        matchup.player2_osu_id.clone(),
        matchup.player2_username.clone(),
        user.osu_id.clone(),
    )
    .await?;

    // 模式 3：发送邀请消息给对手并置 scheduled
    if body.send_notification {
        let (opp_id, opp_name) = if matchup.player1_osu_id == user.osu_id {
            (
                matchup.player2_osu_id.clone(),
                matchup.player2_username.clone(),
            )
        } else {
            (
                matchup.player1_osu_id.clone(),
                matchup.player1_username.clone(),
            )
        };
        let now = Utc::now().naive_utc();
        let content = format!(
            "{} 已预约与你在 {} 的对战。请确认是否接受。",
            user.username, room.room_name
        );
        let message_id = run_db(&state.pool, move |conn| {
            diesel::insert_into(messages::table)
                .values((
                    messages::sender_osu_id.eq(&user.osu_id),
                    messages::sender_username.eq(&user.username),
                    messages::receiver_osu_id.eq(&opp_id),
                    messages::receiver_username.eq(&opp_name),
                    messages::message_type.eq(crate::enums::MessageType::MatchInvitation),
                    messages::title.eq("对战预约确认"),
                    messages::content.eq(content),
                    messages::related_matchup_id.eq(matchup.id),
                    messages::status.eq(crate::enums::MessageStatus::Unread),
                    messages::created_at.eq(now),
                    messages::updated_at.eq(now),
                ))
                .returning(messages::id)
                .get_result::<i32>(conn)
                .map_err(AppError::from)
        })
        .await?;
        set_matchup_status(&state, matchup.id, MatchupStatus::Scheduled).await?;
        return Ok(ok_msg(
            "预约成功，已发送确认请求给对手",
            json!({ "schedule": schedule.to_json(None), "messageId": message_id }),
        ));
    }

    // 模式 2：普通创建
    set_matchup_status(&state, matchup.id, MatchupStatus::Scheduled).await?;
    Ok(ok_msg("比赛预约创建成功", schedule.to_json(None)))
}

async fn insert_schedule(
    state: &AppState,
    room_id: i32,
    p1: String,
    p1u: String,
    p2: String,
    p2u: String,
    created_by: String,
) -> Result<MatchSchedule, AppError> {
    run_db(&state.pool, move |conn| {
        let now = Utc::now().naive_utc();
        diesel::insert_into(match_schedules::table)
            .values((
                match_schedules::room_id.eq(room_id),
                match_schedules::player1_osu_id.eq(&p1),
                match_schedules::player1_username.eq(&p1u),
                match_schedules::player2_osu_id.eq(&p2),
                match_schedules::player2_username.eq(&p2u),
                match_schedules::red_score.eq(0),
                match_schedules::blue_score.eq(0),
                match_schedules::status.eq(ScheduleStatus::Pending),
                match_schedules::created_by.eq(&created_by),
                match_schedules::created_at.eq(now),
                match_schedules::updated_at.eq(now),
            ))
            .get_result::<MatchSchedule>(conn)
            .map_err(AppError::from)
    })
    .await
}

async fn set_matchup_status(
    state: &AppState,
    id: i32,
    status: MatchupStatus,
) -> Result<(), AppError> {
    run_db(&state.pool, move |conn| {
        diesel::update(player_matchups::table.find(id))
            .set((
                player_matchups::status.eq(status),
                player_matchups::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(())
}

// ============ PATCH /api/schedules/{id} ============

#[derive(serde::Deserialize)]
struct UpdateStatusBody {
    status: String,
}

async fn update_schedule_status(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdateStatusBody>,
) -> ApiResult<Json<Value>> {
    let new_status =
        ScheduleStatus::parse(&body.status).ok_or_else(|| AppError::bad_request("无效的状态"))?;
    let permissions = permissions_json(&state, &user.osu_id).await?;
    let is_admin = permissions["isadmin"].as_bool().unwrap_or(false);

    if !is_admin {
        // 玩家：必须是参与者
        let schedule = run_db(&state.pool, move |conn| {
            match_schedules::table
                .find(id)
                .first::<MatchSchedule>(conn)
                .optional()
                .map_err(AppError::from)
        })
        .await?
        .ok_or_else(|| AppError::not_found("比赛预约不存在"))?;
        if schedule.player1_osu_id != user.osu_id && schedule.player2_osu_id != user.osu_id {
            return Err(AppError::forbidden("无权限修改此比赛预约"));
        }
        // 只能 pending → confirmed/cancelled
        if !matches!(
            new_status,
            ScheduleStatus::Confirmed | ScheduleStatus::Cancelled
        ) {
            return Err(AppError::forbidden("无权限执行此操作"));
        }
        if schedule.status != ScheduleStatus::Pending {
            return Err(AppError::bad_request("只能修改待确认状态的预约"));
        }
        // 创建者不能确认自己的预约
        if new_status == ScheduleStatus::Confirmed && schedule.created_by == user.osu_id {
            return Err(AppError::forbidden("您不能确认自己创建的比赛预约"));
        }
    }

    let n = run_db(&state.pool, move |conn| {
        diesel::update(match_schedules::table.find(id))
            .set((
                match_schedules::status.eq(new_status),
                match_schedules::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("比赛预约不存在"));
    }
    Ok(ok_msg("比赛预约更新成功", Value::Null))
}

// ============ PATCH /api/schedules/{id}/details ============

#[derive(serde::Deserialize)]
struct UpdateDetailsBody {
    #[serde(rename = "redScore", alias = "red_score", default)]
    red_score: Option<i32>,
    #[serde(rename = "blueScore", alias = "blue_score", default)]
    blue_score: Option<i32>,
    #[serde(rename = "matchLink", alias = "match_link", default)]
    match_link: Option<String>,
    #[serde(rename = "replayLink", alias = "replay_link", default)]
    replay_link: Option<String>,
    #[serde(rename = "streamLink", alias = "stream_link", default)]
    stream_link: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

async fn update_schedule_details(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdateDetailsBody>,
) -> ApiResult<Json<Value>> {
    let status = match &body.status {
        Some(s) => {
            Some(ScheduleStatus::parse(s).ok_or_else(|| AppError::bad_request("无效的状态"))?)
        }
        None => None,
    };
    let n = run_db(&state.pool, move |conn| {
        diesel::update(match_schedules::table.find(id))
            .set((
                body.red_score.map(|v| match_schedules::red_score.eq(v)),
                body.blue_score.map(|v| match_schedules::blue_score.eq(v)),
                body.match_link
                    .clone()
                    .map(|v| match_schedules::match_link.eq(v)),
                body.replay_link
                    .clone()
                    .map(|v| match_schedules::replay_link.eq(v)),
                body.stream_link
                    .clone()
                    .map(|v| match_schedules::stream_link.eq(v)),
                status.map(|v| match_schedules::status.eq(v)),
                Some(match_schedules::updated_at.eq(Utc::now().naive_utc())),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("比赛预约不存在"));
    }
    Ok(ok_msg("比赛信息更新成功", Value::Null))
}
