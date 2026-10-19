// 消息路由（原 messages：GET 列表 + POST 状态/对战响应动作）
//
//   GET  /api/messages                     我收到的消息
//   POST /api/messages {messageId, action} action=read|accept|decline
//        accept → 关联对战置 scheduled；decline → 关联对战置回 available
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::run_db;
use crate::enums::{MatchupStatus, MessageStatus, ScheduleStatus};
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::{Message, PlayerMatchup};
use crate::schema::{match_schedules, messages, player_matchups};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/messages",
        get(list_messages).post(handle_message_action),
    )
}

// ============ GET /api/messages ============

async fn list_messages(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
) -> ApiResult<Json<Value>> {
    let list = run_db(&state.pool, move |conn| {
        messages::table
            .filter(messages::receiver_osu_id.eq(&user.osu_id))
            .order(messages::created_at.desc())
            .load::<Message>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok(json!(
        list.iter().map(|m| m.to_json()).collect::<Vec<_>>()
    )))
}

// ============ POST /api/messages ============

#[derive(serde::Deserialize)]
struct MessageActionBody {
    #[serde(rename = "messageId", alias = "message_id")]
    message_id: i64,
    action: String,
}

async fn handle_message_action(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Json(body): Json<MessageActionBody>,
) -> ApiResult<Json<Value>> {
    if !matches!(body.action.as_str(), "read" | "accept" | "decline") {
        return Err(AppError::bad_request("无效的操作类型"));
    }
    let message_id =
        i32::try_from(body.message_id).map_err(|_| AppError::bad_request("无效的消息ID"))?;

    let message = load_message(&state, message_id).await?;
    apply_message_action(&state, &message, &user.osu_id, &body.action).await?;
    Ok(ok_msg("消息状态更新成功", Value::Null))
}

/// 读取消息（不存在时 404）
pub(crate) async fn load_message(state: &AppState, message_id: i32) -> Result<Message, AppError> {
    run_db(&state.pool, move |conn| {
        messages::table
            .find(message_id)
            .first::<Message>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?
    .ok_or_else(|| AppError::not_found("消息不存在"))
}

/// 校验消息归属并处理状态/邀请动作（read/accept/decline 均置已读）。
/// accept：创建赛程并对战置 scheduled；decline：对战回到 available。
pub(crate) async fn apply_message_action(
    state: &AppState,
    message: &Message,
    actor_osu_id: &str,
    action: &str,
) -> Result<(), AppError> {
    if message.receiver_osu_id != actor_osu_id {
        return Err(AppError::forbidden("无权操作此消息"));
    }
    let message_id = message.id;
    run_db(&state.pool, move |conn| {
        diesel::update(messages::table.find(message_id))
            .set((
                messages::status.eq(MessageStatus::Read),
                messages::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;

    if !matches!(action, "accept" | "decline") {
        return Ok(());
    }
    let Some(matchup_id) = message.related_matchup_id else {
        return Ok(());
    };
    let matchup = run_db(&state.pool, move |conn| {
        player_matchups::table
            .find(matchup_id)
            .first::<PlayerMatchup>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;
    let Some(mu) = matchup else {
        return Ok(());
    };
    // 双保险：即使消息里的 related_matchup_id 被写错，也只能操作自己参与的对战
    if mu.player1_osu_id != actor_osu_id && mu.player2_osu_id != actor_osu_id {
        return Err(AppError::forbidden("您不是该对战的参赛选手"));
    }

    let is_pending = mu.status == MatchupStatus::Pending;
    if action == "accept" {
        if is_pending {
            let room_id = mu
                .room_id
                .ok_or_else(|| AppError::bad_request("缺少预约房间，无法创建赛程"))?;
            insert_schedule_from_matchup(state, &mu, room_id, actor_osu_id.to_string()).await?;
        }
        // 置 scheduled（历史邀请已是 scheduled 则幂等跳过创建）
        run_db(&state.pool, move |conn| {
            diesel::update(player_matchups::table.find(matchup_id))
                .set((
                    player_matchups::status.eq(MatchupStatus::Scheduled),
                    player_matchups::updated_at.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)
                .map_err(AppError::from)
        })
        .await?;
    } else if is_pending {
        // decline：回到可预约，并清除绑定的房间
        run_db(&state.pool, move |conn| {
            diesel::update(player_matchups::table.find(matchup_id))
                .set((
                    player_matchups::status.eq(MatchupStatus::Available),
                    player_matchups::room_id.eq(None::<i32>),
                    player_matchups::updated_at.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)
                .map_err(AppError::from)
        })
        .await?;
    }
    Ok(())
}

/// 接受邀请后按对战绑定的房间创建赛程（pending，等待 staff 最终确认）
pub(crate) async fn insert_schedule_from_matchup(
    state: &AppState,
    mu: &PlayerMatchup,
    room_id: i32,
    created_by: String,
) -> Result<(), AppError> {
    let p1 = mu.player1_osu_id.clone();
    let p1u = mu.player1_username.clone();
    let p2 = mu.player2_osu_id.clone();
    let p2u = mu.player2_username.clone();
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
            .execute(conn)
            .map_err(AppError::from)?;
        Ok(())
    })
    .await
}
