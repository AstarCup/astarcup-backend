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
    entities::{
        multiplayer_room, multiplayer_solo_room, multiplayer_teamvs_room, prelude::*, team, user,
    },
    utils::response::{ApiResponse, ApiResult},
};

#[derive(Debug, Deserialize)]
pub struct GetMatchRoomsQuery {
    page: Option<u64>,
    limit: Option<u64>,
    room_type: Option<String>,
    season: Option<String>,
    category: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MatchRoomResponse {
    room_id: i32,
    room_type: String,
    season: String,
    category: String,
    multiplayer_solo_room_room_id: Option<i32>,
    multiplayer_teamvs_room_room_id: Option<i32>,
    score_red: i32,
    score_blue: i32,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: chrono::DateTime<chrono::Utc>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<multiplayer_room::Model> for MatchRoomResponse {
    fn from(model: multiplayer_room::Model) -> Self {
        Self {
            room_id: model.room_id,
            room_type: model.r#type.to_string(),
            season: model.season.to_string(),
            category: model.category.to_string(),
            multiplayer_solo_room_room_id: model.multiplayer_solo_room_room_id,
            multiplayer_teamvs_room_room_id: model.multiplayer_teamvs_room_room_id,
            score_red: model.score_red,
            score_blue: model.score_blue,
            start_time: model.start_time,
            end_time: model.end_time,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateMatchRoomRequest {
    room_type: String,
    season: String,
    category: String,
    multiplayer_solo_room_room_id: Option<i32>,
    multiplayer_teamvs_room_room_id: Option<i32>,
    score_red: i32,
    score_blue: i32,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMatchRoomRequest {
    score_red: Option<i32>,
    score_blue: Option<i32>,
    start_time: Option<chrono::DateTime<chrono::Utc>>,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// 获取比赛房间列表
pub async fn get_match_rooms(
    State(db): State<Database>,
    Query(query): Query<GetMatchRoomsQuery>,
) -> ApiResult<Vec<MatchRoomResponse>> {
    let conn = db.get_connection();

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    let mut query = MultiplayerRoom::find().order_by_desc(multiplayer_room::Column::CreatedAt);

    if let Some(room_type) = query.room_type {
        query = query.filter(multiplayer_room::Column::Type.eq(room_type));
    }

    if let Some(season) = query.season {
        query = query.filter(multiplayer_room::Column::Season.eq(season));
    }

    if let Some(category) = query.category {
        query = query.filter(multiplayer_room::Column::Category.eq(category));
    }

    let rooms = query
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch match rooms: {}", e);
            ApiResponse::internal_server_error("Failed to fetch match rooms")
        })?;

    let response = rooms.into_iter().map(MatchRoomResponse::from).collect();
    Ok(ApiResponse::success(response))
}

/// 获取单个比赛房间
pub async fn get_match_room(
    State(db): State<Database>,
    Path(room_id): Path<i32>,
) -> ApiResult<MatchRoomResponse> {
    let conn = db.get_connection();

    let room = MultiplayerRoom::find_by_id(room_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch match room: {}", e);
            ApiResponse::internal_server_error("Failed to fetch match room")
        })?
        .ok_or_else(|| ApiResponse::not_found("Match room not found"))?;

    Ok(ApiResponse::success(MatchRoomResponse::from(room)))
}

/// 创建比赛房间
pub async fn create_match_room(
    State(db): State<Database>,
    Json(payload): Json<CreateMatchRoomRequest>,
) -> ApiResult<MatchRoomResponse> {
    let conn = db.get_connection();

    // 验证房间类型
    let room_type = match payload.room_type.as_str() {
        "SOLO" => multiplayer_room::MultiplayerRoomType::Solo,
        "TEAM_VS" => multiplayer_room::MultiplayerRoomType::TeamVs,
        _ => return Err(ApiResponse::bad_request("Invalid room type")),
    };

    // 验证赛季
    let season = match payload.season.as_str() {
        "S1" => multiplayer_room::Season::S1,
        "S2" => multiplayer_room::Season::S2,
        _ => return Err(ApiResponse::bad_request("Invalid season")),
    };

    // 验证类别
    let category = match payload.category.as_str() {
        "QUA" => multiplayer_room::Category::Qua,
        "RO16" => multiplayer_room::Category::Ro16,
        "QF" => multiplayer_room::Category::Qf,
        "SF" => multiplayer_room::Category::Sf,
        "F" => multiplayer_room::Category::F,
        "GF" => multiplayer_room::Category::Gf,
        _ => return Err(ApiResponse::bad_request("Invalid category")),
    };

    // 创建房间
    let room = multiplayer_room::ActiveModel {
        r#type: Set(room_type),
        season: Set(season),
        category: Set(category),
        multiplayer_solo_room_room_id: Set(payload.multiplayer_solo_room_room_id),
        multiplayer_teamvs_room_room_id: Set(payload.multiplayer_teamvs_room_room_id),
        score_red: Set(payload.score_red),
        score_blue: Set(payload.score_blue),
        start_time: Set(payload.start_time),
        end_time: Set(payload.end_time),
        ..Default::default()
    };

    let room = room.insert(conn).await.map_err(|e| {
        tracing::error!("Failed to create match room: {}", e);
        ApiResponse::internal_server_error("Failed to create match room")
    })?;

    Ok(ApiResponse::success(MatchRoomResponse::from(room)))
}

/// 更新比赛房间
pub async fn update_match_room(
    State(db): State<Database>,
    Path(room_id): Path<i32>,
    Json(payload): Json<UpdateMatchRoomRequest>,
) -> ApiResult<MatchRoomResponse> {
    let conn = db.get_connection();

    let mut room: multiplayer_room::ActiveModel = MultiplayerRoom::find_by_id(room_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch match room: {}", e);
            ApiResponse::internal_server_error("Failed to fetch match room")
        })?
        .ok_or_else(|| ApiResponse::not_found("Match room not found"))?
        .into();

    if let Some(score_red) = payload.score_red {
        room.score_red = Set(score_red);
    }

    if let Some(score_blue) = payload.score_blue {
        room.score_blue = Set(score_blue);
    }

    if let Some(start_time) = payload.start_time {
        room.start_time = Set(start_time);
    }

    if let Some(end_time) = payload.end_time {
        room.end_time = Set(end_time);
    }

    let room = room.update(conn).await.map_err(|e| {
        tracing::error!("Failed to update match room: {}", e);
        ApiResponse::internal_server_error("Failed to update match room")
    })?;

    Ok(ApiResponse::success(MatchRoomResponse::from(room)))
}

/// 删除比赛房间
pub async fn delete_match_room(
    State(db): State<Database>,
    Path(room_id): Path<i32>,
) -> ApiResult<()> {
    let conn = db.get_connection();

    let room = MultiplayerRoom::find_by_id(room_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch match room: {}", e);
            ApiResponse::internal_server_error("Failed to fetch match room")
        })?
        .ok_or_else(|| ApiResponse::not_found("Match room not found"))?;

    MultiplayerRoom::delete_by_id(room.room_id)
        .exec(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete match room: {}", e);
            ApiResponse::internal_server_error("Failed to delete match room")
        })?;

    Ok(ApiResponse::success_message(
        "Match room deleted successfully",
    ))
}

/// 获取比赛房间总数
pub async fn get_match_rooms_count(
    State(db): State<Database>,
    Query(query): Query<GetMatchRoomsQuery>,
) -> ApiResult<u64> {
    let conn = db.get_connection();

    let mut query = MultiplayerRoom::find();

    if let Some(room_type) = query.room_type {
        query = query.filter(multiplayer_room::Column::Type.eq(room_type));
    }

    if let Some(season) = query.season {
        query = query.filter(multiplayer_room::Column::Season.eq(season));
    }

    if let Some(category) = query.category {
        query = query.filter(multiplayer_room::Column::Category.eq(category));
    }

    let count = query.count(conn).await.map_err(|e| {
        tracing::error!("Failed to count match rooms: {}", e);
        ApiResponse::internal_server_error("Failed to count match rooms")
    })?;

    Ok(ApiResponse::success(count))
}
