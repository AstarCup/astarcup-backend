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
use crate::enums::{MatchupStatus, MessageStatus};
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::Message;
use crate::schema::{messages, player_matchups};
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
    _user: crate::auth::SessionUser,
    Json(body): Json<MessageActionBody>,
) -> ApiResult<Json<Value>> {
    if !matches!(body.action.as_str(), "read" | "accept" | "decline") {
        return Err(AppError::bad_request("无效的操作类型"));
    }
    let message_id =
        i32::try_from(body.message_id).map_err(|_| AppError::bad_request("无效的消息ID"))?;

    // 更新消息状态（read/accept/decline 均置已读）
    let n = run_db(&state.pool, move |conn| {
        diesel::update(messages::table.find(message_id))
            .set((
                messages::status.eq(MessageStatus::Read),
                messages::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("消息不存在或更新失败"));
    }

    // accept/decline 时联动对战状态
    if matches!(body.action.as_str(), "accept" | "decline") {
        let msg = run_db(&state.pool, move |conn| {
            messages::table
                .find(message_id)
                .first::<Message>(conn)
                .optional()
                .map_err(AppError::from)
        })
        .await?;
        if let Some(m) = msg {
            if let Some(matchup_id) = m.related_matchup_id {
                let new_status = if body.action == "accept" {
                    MatchupStatus::Scheduled
                } else {
                    MatchupStatus::Available
                };
                run_db(&state.pool, move |conn| {
                    diesel::update(player_matchups::table.find(matchup_id))
                        .set((
                            player_matchups::status.eq(new_status),
                            player_matchups::updated_at.eq(Utc::now().naive_utc()),
                        ))
                        .execute(conn)
                        .map_err(AppError::from)
                })
                .await?;
            }
        }
    }
    Ok(ok_msg("消息状态更新成功", Value::Null))
}
