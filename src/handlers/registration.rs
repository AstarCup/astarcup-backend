use axum::{
    Json,
    extract::{Path, Query, State},
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    entities::{prelude::*, user},
    utils::response::{ApiResponse, ApiResult},
};

#[derive(Debug, Deserialize)]
pub struct GetRegistrationsQuery {
    page: Option<u64>,
    limit: Option<u64>,
    approved: Option<bool>,
    seasonal: Option<String>,
    user_state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegistrationResponse {
    id: i32,
    osuid: i32,
    username: String,
    avatar_url: Option<String>,
    cover_url: Option<String>,
    country_code: String,
    pp: f64,
    global_rank: i32,
    country_rank: i32,
    approved: bool,
    seed: i32,
    seasonal: String,
    user_state: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<user::Model> for RegistrationResponse {
    fn from(model: user::Model) -> Self {
        Self {
            id: model.id,
            osuid: model.osuid,
            username: model.username,
            avatar_url: model.avatar_url,
            cover_url: model.cover_url,
            country_code: model.country_code,
            pp: model.pp,
            global_rank: model.global_rank,
            country_rank: model.country_rank,
            approved: model.approved,
            seed: model.seed,
            seasonal: model.seasonal.into(),
            user_state: model.user_state.to_string(),
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ApproveRegistrationRequest {
    approved: bool,
}

/// 获取报名列表
pub async fn get_registrations(
    State(db): State<Database>,
    Query(query): Query<GetRegistrationsQuery>,
) -> ApiResult<Vec<RegistrationResponse>> {
    let conn = db.get_connection();

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    let mut query = User::find().order_by_asc(user::Column::Id);

    if let Some(approved) = query.approved {
        query = query.filter(user::Column::Approved.eq(approved));
    }

    if let Some(seasonal) = query.seasonal {
        query = query.filter(user::Column::Seasonal.eq(seasonal));
    }

    if let Some(user_state) = query.user_state {
        query = query.filter(user::Column::UserState.eq(user_state));
    }

    let users = query
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch registrations: {}", e);
            ApiResponse::internal_server_error("Failed to fetch registrations")
        })?;

    let response = users.into_iter().map(RegistrationResponse::from).collect();
    Ok(ApiResponse::success(response))
}

/// 获取已批准的玩家
pub async fn get_approved_players(
    State(db): State<Database>,
    Query(query): Query<GetRegistrationsQuery>,
) -> ApiResult<Vec<RegistrationResponse>> {
    let conn = db.get_connection();

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    let users = User::find()
        .filter(user::Column::Approved.eq(true))
        .order_by_asc(user::Column::Id)
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch approved players: {}", e);
            ApiResponse::internal_server_error("Failed to fetch approved players")
        })?;

    let response = users.into_iter().map(RegistrationResponse::from).collect();
    Ok(ApiResponse::success(response))
}

/// 批准或拒绝报名
pub async fn approve_registration(
    State(db): State<Database>,
    Path(user_id): Path<i32>,
    Json(payload): Json<ApproveRegistrationRequest>,
) -> ApiResult<RegistrationResponse> {
    let conn = db.get_connection();

    let mut user: user::ActiveModel = User::find_by_id(user_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user: {}", e);
            ApiResponse::internal_server_error("Failed to fetch user")
        })?
        .ok_or_else(|| ApiResponse::not_found("User not found"))?
        .into();

    user.approved = sea_orm::Set(payload.approved);

    let user = user.update(conn).await.map_err(|e| {
        tracing::error!("Failed to update user approval status: {}", e);
        ApiResponse::internal_server_error("Failed to update approval status")
    })?;

    Ok(ApiResponse::success(RegistrationResponse::from(user)))
}

/// 删除报名
pub async fn delete_registration(
    State(db): State<Database>,
    Path(user_id): Path<i32>,
) -> ApiResult<()> {
    let conn = db.get_connection();

    let user = User::find_by_id(user_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user: {}", e);
            ApiResponse::internal_server_error("Failed to fetch user")
        })?
        .ok_or_else(|| ApiResponse::not_found("User not found"))?;

    // 软删除：将用户状态设置为BANNED
    let mut user_model: user::ActiveModel = user.into();
    user_model.user_state = sea_orm::Set(user::UserState::Banned);

    user_model.update(conn).await.map_err(|e| {
        tracing::error!("Failed to delete registration: {}", e);
        ApiResponse::internal_server_error("Failed to delete registration")
    })?;

    Ok(ApiResponse::success_message(
        "Registration deleted successfully",
    ))
}

/// 获取报名总数
pub async fn get_registrations_count(
    State(db): State<Database>,
    Query(query): Query<GetRegistrationsQuery>,
) -> ApiResult<u64> {
    let conn = db.get_connection();

    let mut query = User::find();

    if let Some(approved) = query.approved {
        query = query.filter(user::Column::Approved.eq(approved));
    }

    if let Some(seasonal) = query.seasonal {
        query = query.filter(user::Column::Seasonal.eq(seasonal));
    }

    if let Some(user_state) = query.user_state {
        query = query.filter(user::Column::UserState.eq(user_state));
    }

    let count = query.count(conn).await.map_err(|e| {
        tracing::error!("Failed to count registrations: {}", e);
        ApiResponse::internal_server_error("Failed to count registrations")
    })?;

    Ok(ApiResponse::success(count))
}
