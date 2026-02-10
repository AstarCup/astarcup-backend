use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sea_orm::{DatabaseConnection, EntityTrait, PaginatorTrait, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    entities::{prelude::*, user},
    utils::response::{ApiResponse, ApiResult},
};

#[derive(Debug, Deserialize)]
pub struct GetUsersQuery {
    page: Option<u64>,
    limit: Option<u64>,
    approved: Option<bool>,
    seasonal: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
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

impl From<user::Model> for UserResponse {
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

/// 获取用户列表
pub async fn get_users(
    State(db): State<Database>,
    Query(query): Query<GetUsersQuery>,
) -> ApiResult<Vec<UserResponse>> {
    let conn = db.get_connection();
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;
    
    let mut query = User::find()
        .order_by_asc(user::Column::Id);
    
    if let Some(approved) = query.approved {
        query = query.filter(user::Column::Approved.eq(approved));
    }
    
    if let Some(seasonal) = query.seasonal {
        query = query.filter(user::Column::Seasonal.eq(seasonal));
    }
    
    let users = query
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch users: {}", e);
            ApiResponse::internal_server_error("Failed to fetch users")
        })?;
    
    let response = users.into_iter().map(UserResponse::from).collect();
    Ok(ApiResponse::success(response))
}

/// 获取单个用户
pub async fn get_user(
    State(db): State<Database>,
    Path(user_id): Path<i32>,
) -> ApiResult<UserResponse> {
    let conn = db.get_connection();
    
    let user = User::find_by_id(user_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user: {}", e);
            ApiResponse::internal_server_error("Failed to fetch user")
        })?
        .ok_or_else(|| ApiResponse::not_found("User not found"))?;
    
    Ok(ApiResponse::success(UserResponse::from(user)))
}

/// 获取用户总数
pub async fn get_users_count(
    State(db): State<Database>,
    Query(query): Query<GetUsersQuery>,
) -> ApiResult<u64> {
    let conn = db.get_connection();
    
    let mut query = User::find();
    
    if let Some(approved) = query.approved {
        query = query.filter(user::Column::Approved.eq(approved));
    }
    
    if let Some(seasonal) = query.seasonal {
        query = query.filter(user::Column::Seasonal.eq(seasonal));
    }
    
    let count = query
        .count(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count users: {}", e);
            ApiResponse::internal_server_error("Failed to count users")
        })?;
    
    Ok(ApiResponse::success(count))
}

/// 通过osu ID获取用户
pub async fn get_user_by_osuid(
    State(db): State<Database>,
    Path(osuid): Path<i32>,
) -> ApiResult<UserResponse> {
    let conn = db.get_connection();
    
    let user = User::find()
        .filter(user::Column::Osuid.eq(osuid))
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user by osuid: {}", e);
            ApiResponse::internal_server_error("Failed to fetch user")
        })?
        .ok_or_else(|| ApiResponse::not_found("User not found"))?;
    
    Ok(ApiResponse::success(UserResponse::from(user)))
}