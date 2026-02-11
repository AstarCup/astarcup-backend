use axum::{
    Json,
    extract::{Path, Query, State},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    entities::{message, multiplayer_room, prelude::*, user},
    utils::response::{ApiResponse, ApiResult},
};

#[derive(Debug, Deserialize)]
pub struct GetMessagesQuery {
    page: Option<u64>,
    limit: Option<u64>,
    receiver_id: Option<i32>,
    sender_id: Option<i32>,
    room_id: Option<i32>,
    is_read: Option<bool>,
    is_system_message: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    id: i32,
    title: String,
    content: String,
    sender_id: i32,
    receiver_id: i32,
    room_id: Option<i32>,
    is_read: bool,
    is_deleted: bool,
    is_system_message: bool,
    is_accepted: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<message::Model> for MessageResponse {
    fn from(model: message::Model) -> Self {
        Self {
            id: model.id,
            title: model.title,
            content: model.content,
            sender_id: model.sender_id,
            receiver_id: model.receiver_id,
            room_id: model.room_id,
            is_read: model.is_read,
            is_deleted: model.is_deleted,
            is_system_message: model.is_system_message,
            is_accepted: model.is_accepted,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    title: String,
    content: String,
    receiver_id: i32,
    room_id: Option<i32>,
    is_system_message: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMessageRequest {
    is_read: Option<bool>,
    is_accepted: Option<bool>,
}

/// 获取消息列表
pub async fn get_messages(
    State(db): State<Database>,
    Query(query): Query<GetMessagesQuery>,
) -> ApiResult<Vec<MessageResponse>> {
    let conn = db.get_connection();

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    let mut query = Message::find()
        .filter(message::Column::IsDeleted.eq(false))
        .order_by_desc(message::Column::CreatedAt);

    if let Some(receiver_id) = query.receiver_id {
        query = query.filter(message::Column::ReceiverId.eq(receiver_id));
    }

    if let Some(sender_id) = query.sender_id {
        query = query.filter(message::Column::SenderId.eq(sender_id));
    }

    if let Some(room_id) = query.room_id {
        query = query.filter(message::Column::RoomId.eq(room_id));
    }

    if let Some(is_read) = query.is_read {
        query = query.filter(message::Column::IsRead.eq(is_read));
    }

    if let Some(is_system_message) = query.is_system_message {
        query = query.filter(message::Column::IsSystemMessage.eq(is_system_message));
    }

    let messages = query
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch messages: {}", e);
            ApiResponse::internal_server_error("Failed to fetch messages")
        })?;

    let response = messages.into_iter().map(MessageResponse::from).collect();
    Ok(ApiResponse::success(response))
}

/// 获取单个消息
pub async fn get_message(
    State(db): State<Database>,
    Path(message_id): Path<i32>,
) -> ApiResult<MessageResponse> {
    let conn = db.get_connection();

    let message = Message::find_by_id(message_id)
        .filter(message::Column::IsDeleted.eq(false))
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch message: {}", e);
            ApiResponse::internal_server_error("Failed to fetch message")
        })?
        .ok_or_else(|| ApiResponse::not_found("Message not found"))?;

    Ok(ApiResponse::success(MessageResponse::from(message)))
}

/// 发送消息
pub async fn send_message(
    State(db): State<Database>,
    Json(payload): Json<CreateMessageRequest>,
) -> ApiResult<MessageResponse> {
    let conn = db.get_connection();

    // 验证接收者是否存在
    let receiver = User::find()
        .filter(user::Column::Osuid.eq(payload.receiver_id))
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to verify receiver: {}", e);
            ApiResponse::internal_server_error("Failed to verify receiver")
        })?
        .ok_or_else(|| ApiResponse::not_found("Receiver not found"))?;

    // 验证房间是否存在（如果提供了room_id）
    if let Some(room_id) = payload.room_id {
        let room = MultiplayerRoom::find_by_id(room_id)
            .one(conn)
            .await
            .map_err(|e| {
                tracing::error!("Failed to verify room: {}", e);
                ApiResponse::internal_server_error("Failed to verify room")
            })?;

        if room.is_none() {
            return Err(ApiResponse::not_found("Room not found"));
        }
    }

    // 创建消息
    let message = message::ActiveModel {
        title: Set(payload.title),
        content: Set(payload.content),
        receiver_id: Set(payload.receiver_id),
        room_id: Set(payload.room_id),
        is_system_message: Set(payload.is_system_message.unwrap_or(false)),
        ..Default::default()
    };

    let message = message.insert(conn).await.map_err(|e| {
        tracing::error!("Failed to send message: {}", e);
        ApiResponse::internal_server_error("Failed to send message")
    })?;

    Ok(ApiResponse::success(MessageResponse::from(message)))
}

/// 更新消息状态（标记为已读/接受）
pub async fn update_message(
    State(db): State<Database>,
    Path(message_id): Path<i32>,
    Json(payload): Json<UpdateMessageRequest>,
) -> ApiResult<MessageResponse> {
    let conn = db.get_connection();

    let mut message: message::ActiveModel = Message::find_by_id(message_id)
        .filter(message::Column::IsDeleted.eq(false))
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch message: {}", e);
            ApiResponse::internal_server_error("Failed to fetch message")
        })?
        .ok_or_else(|| ApiResponse::not_found("Message not found"))?
        .into();

    if let Some(is_read) = payload.is_read {
        message.is_read = Set(is_read);
    }

    if let Some(is_accepted) = payload.is_accepted {
        message.is_accepted = Set(is_accepted);
    }

    let message = message.update(conn).await.map_err(|e| {
        tracing::error!("Failed to update message: {}", e);
        ApiResponse::internal_server_error("Failed to update message")
    })?;

    Ok(ApiResponse::success(MessageResponse::from(message)))
}

/// 删除消息（软删除）
pub async fn delete_message(
    State(db): State<Database>,
    Path(message_id): Path<i32>,
) -> ApiResult<()> {
    let conn = db.get_connection();

    let mut message: message::ActiveModel = Message::find_by_id(message_id)
        .filter(message::Column::IsDeleted.eq(false))
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch message: {}", e);
            ApiResponse::internal_server_error("Failed to fetch message")
        })?
        .ok_or_else(|| ApiResponse::not_found("Message not found"))?
        .into();

    message.is_deleted = Set(true);

    message.update(conn).await.map_err(|e| {
        tracing::error!("Failed to delete message: {}", e);
        ApiResponse::internal_server_error("Failed to delete message")
    })?;

    Ok(ApiResponse::success_message("Message deleted successfully"))
}

/// 获取消息总数
pub async fn get_messages_count(
    State(db): State<Database>,
    Query(query): Query<GetMessagesQuery>,
) -> ApiResult<u64> {
    let conn = db.get_connection();

    let mut query = Message::find().filter(message::Column::IsDeleted.eq(false));

    if let Some(receiver_id) = query.receiver_id {
        query = query.filter(message::Column::ReceiverId.eq(receiver_id));
    }

    if let Some(sender_id) = query.sender_id {
        query = query.filter(message::Column::SenderId.eq(sender_id));
    }

    if let Some(room_id) = query.room_id {
        query = query.filter(message::Column::RoomId.eq(room_id));
    }

    if let Some(is_read) = query.is_read {
        query = query.filter(message::Column::IsRead.eq(is_read));
    }

    if let Some(is_system_message) = query.is_system_message {
        query = query.filter(message::Column::IsSystemMessage.eq(is_system_message));
    }

    let count = query.count(conn).await.map_err(|e| {
        tracing::error!("Failed to count messages: {}", e);
        ApiResponse::internal_server_error("Failed to count messages")
    })?;

    Ok(ApiResponse::success(count))
}

/// 获取未读消息数量
pub async fn get_unread_messages_count(
    State(db): State<Database>,
    Path(receiver_id): Path<i32>,
) -> ApiResult<u64> {
    let conn = db.get_connection();

    let count = Message::find()
        .filter(message::Column::ReceiverId.eq(receiver_id))
        .filter(message::Column::IsRead.eq(false))
        .filter(message::Column::IsDeleted.eq(false))
        .count(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count unread messages: {}", e);
            ApiResponse::internal_server_error("Failed to count unread messages")
        })?;

    Ok(ApiResponse::success(count))
}
