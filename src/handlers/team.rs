use axum::{
    extract::{Path, Query, State},
    Json,
};
use sea_orm::{DatabaseConnection, EntityTrait, PaginatorTrait, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    entities::{prelude::*, team},
    utils::response::{ApiResponse, ApiResult},
};

#[derive(Debug, Deserialize)]
pub struct GetTeamsQuery {
    page: Option<u64>,
    limit: Option<u64>,
    team_state: Option<String>,
    team_color: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TeamResponse {
    id: i32,
    name: String,
    banner_url: String,
    team_color: String,
    team_state: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<team::Model> for TeamResponse {
    fn from(model: team::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            banner_url: model.banner_url,
            team_color: model.team_color.to_string(),
            team_state: model.team_state.to_string(),
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// 获取队伍列表
pub async fn get_teams(
    State(db): State<Database>,
    Query(query): Query<GetTeamsQuery>,
) -> ApiResult<Vec<TeamResponse>> {
    let conn = db.get_connection();
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;
    
    let mut query = Team::find()
        .order_by_asc(team::Column::Id);
    
    if let Some(team_state) = query.team_state {
        query = query.filter(team::Column::TeamState.eq(team_state));
    }
    
    if let Some(team_color) = query.team_color {
        query = query.filter(team::Column::TeamColor.eq(team_color));
    }
    
    let teams = query
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch teams: {}", e);
            ApiResponse::internal_server_error("Failed to fetch teams")
        })?;
    
    let response = teams.into_iter().map(TeamResponse::from).collect();
    Ok(ApiResponse::success(response))
}

/// 获取单个队伍
pub async fn get_team(
    State(db): State<Database>,
    Path(team_id): Path<i32>,
) -> ApiResult<TeamResponse> {
    let conn = db.get_connection();
    
    let team = Team::find_by_id(team_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch team: {}", e);
            ApiResponse::internal_server_error("Failed to fetch team")
        })?
        .ok_or_else(|| ApiResponse::not_found("Team not found"))?;
    
    Ok(ApiResponse::success(TeamResponse::from(team)))
}

/// 获取队伍总数
pub async fn get_teams_count(
    State(db): State<Database>,
    Query(query): Query<GetTeamsQuery>,
) -> ApiResult<u64> {
    let conn = db.get_connection();
    
    let mut query = Team::find();
    
    if let Some(team_state) = query.team_state {
        query = query.filter(team::Column::TeamState.eq(team_state));
    }
    
    if let Some(team_color) = query.team_color {
        query = query.filter(team::Column::TeamColor.eq(team_color));
    }
    
    let count = query
        .count(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count teams: {}", e);
            ApiResponse::internal_server_error("Failed to count teams")
        })?;
    
    Ok(ApiResponse::success(count))
}