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
    entities::{prelude::*, tournament_config},
    utils::response::{ApiResponse, ApiResult},
};

#[derive(Debug, Deserialize)]
pub struct GetTournamentConfigsQuery {
    page: Option<u64>,
    limit: Option<u64>,
    current_seasonal: Option<String>,
    current_category: Option<String>,
    can_register: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TournamentConfigResponse {
    id: i32,
    tournament_name: String,
    max_pp_for_registration: f64,
    min_pp_for_registration: f64,
    current_seasonal: String,
    current_category: String,
    can_register: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<tournament_config::Model> for TournamentConfigResponse {
    fn from(model: tournament_config::Model) -> Self {
        Self {
            id: model.id,
            tournament_name: model.tournament_name,
            max_pp_for_registration: model.max_pp_for_registration,
            min_pp_for_registration: model.min_pp_for_registration,
            current_seasonal: model.current_seasonal.into(),
            current_category: model.current_category.to_string(),
            can_register: model.can_register,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTournamentConfigRequest {
    tournament_name: String,
    max_pp_for_registration: f64,
    min_pp_for_registration: f64,
    current_seasonal: String,
    current_category: String,
    can_register: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTournamentConfigRequest {
    tournament_name: Option<String>,
    max_pp_for_registration: Option<f64>,
    min_pp_for_registration: Option<f64>,
    current_seasonal: Option<String>,
    current_category: Option<String>,
    can_register: Option<bool>,
}

/// 获取比赛设置列表
pub async fn get_tournament_configs(
    State(db): State<Database>,
    Query(query): Query<GetTournamentConfigsQuery>,
) -> ApiResult<Vec<TournamentConfigResponse>> {
    let conn = db.get_connection();

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    let mut query = TournamentConfig::find().order_by_desc(tournament_config::Column::CreatedAt);

    if let Some(current_seasonal) = query.current_seasonal {
        query = query.filter(tournament_config::Column::CurrentSeasonal.eq(current_seasonal));
    }

    if let Some(current_category) = query.current_category {
        query = query.filter(tournament_config::Column::CurrentCategory.eq(current_category));
    }

    if let Some(can_register) = query.can_register {
        query = query.filter(tournament_config::Column::CanRegister.eq(can_register));
    }

    let configs = query
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch tournament configs: {}", e);
            ApiResponse::internal_server_error("Failed to fetch tournament configs")
        })?;

    let response = configs
        .into_iter()
        .map(TournamentConfigResponse::from)
        .collect();
    Ok(ApiResponse::success(response))
}

/// 获取单个比赛设置
pub async fn get_tournament_config(
    State(db): State<Database>,
    Path(config_id): Path<i32>,
) -> ApiResult<TournamentConfigResponse> {
    let conn = db.get_connection();

    let config = TournamentConfig::find_by_id(config_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch tournament config: {}", e);
            ApiResponse::internal_server_error("Failed to fetch tournament config")
        })?
        .ok_or_else(|| ApiResponse::not_found("Tournament config not found"))?;

    Ok(ApiResponse::success(TournamentConfigResponse::from(config)))
}

/// 获取当前比赛设置
pub async fn get_current_tournament_config(
    State(db): State<Database>,
) -> ApiResult<TournamentConfigResponse> {
    let conn = db.get_connection();

    let config = TournamentConfig::find()
        .order_by_desc(tournament_config::Column::CreatedAt)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch current tournament config: {}", e);
            ApiResponse::internal_server_error("Failed to fetch current tournament config")
        })?
        .ok_or_else(|| ApiResponse::not_found("No tournament config found"))?;

    Ok(ApiResponse::success(TournamentConfigResponse::from(config)))
}

/// 创建比赛设置
pub async fn create_tournament_config(
    State(db): State<Database>,
    Json(payload): Json<CreateTournamentConfigRequest>,
) -> ApiResult<TournamentConfigResponse> {
    let conn = db.get_connection();

    // 验证赛季
    let current_seasonal = tournament_config::Season::from(payload.current_seasonal);

    // 验证类别
    let current_category = match payload.current_category.as_str() {
        "QUA" => tournament_config::Category::Qua,
        "RO16" => tournament_config::Category::Ro16,
        "QF" => tournament_config::Category::Qf,
        "SF" => tournament_config::Category::Sf,
        "F" => tournament_config::Category::F,
        "GF" => tournament_config::Category::Gf,
        _ => return Err(ApiResponse::bad_request("Invalid category")),
    };

    // 创建比赛设置
    let config = tournament_config::ActiveModel {
        tournament_name: Set(payload.tournament_name),
        max_pp_for_registration: Set(payload.max_pp_for_registration),
        min_pp_for_registration: Set(payload.min_pp_for_registration),
        current_seasonal: Set(current_seasonal),
        current_category: Set(current_category),
        can_register: Set(payload.can_register),
        ..Default::default()
    };

    let config = config.insert(conn).await.map_err(|e| {
        tracing::error!("Failed to create tournament config: {}", e);
        ApiResponse::internal_server_error("Failed to create tournament config")
    })?;

    Ok(ApiResponse::success(TournamentConfigResponse::from(config)))
}

/// 更新比赛设置
pub async fn update_tournament_config(
    State(db): State<Database>,
    Path(config_id): Path<i32>,
    Json(payload): Json<UpdateTournamentConfigRequest>,
) -> ApiResult<TournamentConfigResponse> {
    let conn = db.get_connection();

    let mut config: tournament_config::ActiveModel = TournamentConfig::find_by_id(config_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch tournament config: {}", e);
            ApiResponse::internal_server_error("Failed to fetch tournament config")
        })?
        .ok_or_else(|| ApiResponse::not_found("Tournament config not found"))?
        .into();

    if let Some(tournament_name) = payload.tournament_name {
        config.tournament_name = Set(tournament_name);
    }

    if let Some(max_pp_for_registration) = payload.max_pp_for_registration {
        config.max_pp_for_registration = Set(max_pp_for_registration);
    }

    if let Some(min_pp_for_registration) = payload.min_pp_for_registration {
        config.min_pp_for_registration = Set(min_pp_for_registration);
    }

    if let Some(current_seasonal) = payload.current_seasonal {
        config.current_seasonal = Set(tournament_config::Season::from(current_seasonal));
    }

    if let Some(current_category) = payload.current_category {
        let category = match current_category.as_str() {
            "QUA" => tournament_config::Category::Qua,
            "RO16" => tournament_config::Category::Ro16,
            "QF" => tournament_config::Category::Qf,
            "SF" => tournament_config::Category::Sf,
            "F" => tournament_config::Category::F,
            "GF" => tournament_config::Category::Gf,
            _ => return Err(ApiResponse::bad_request("Invalid category")),
        };
        config.current_category = Set(category);
    }

    if let Some(can_register) = payload.can_register {
        config.can_register = Set(can_register);
    }

    let config = config.update(conn).await.map_err(|e| {
        tracing::error!("Failed to update tournament config: {}", e);
        ApiResponse::internal_server_error("Failed to update tournament config")
    })?;

    Ok(ApiResponse::success(TournamentConfigResponse::from(config)))
}

/// 删除比赛设置
pub async fn delete_tournament_config(
    State(db): State<Database>,
    Path(config_id): Path<i32>,
) -> ApiResult<()> {
    let conn = db.get_connection();

    let config = TournamentConfig::find_by_id(config_id)
        .one(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch tournament config: {}", e);
            ApiResponse::internal_server_error("Failed to fetch tournament config")
        })?
        .ok_or_else(|| ApiResponse::not_found("Tournament config not found"))?;

    TournamentConfig::delete_by_id(config.id)
        .exec(conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete tournament config: {}", e);
            ApiResponse::internal_server_error("Failed to delete tournament config")
        })?;

    Ok(ApiResponse::success_message(
        "Tournament config deleted successfully",
    ))
}

/// 获取比赛设置总数
pub async fn get_tournament_configs_count(
    State(db): State<Database>,
    Query(query): Query<GetTournamentConfigsQuery>,
) -> ApiResult<u64> {
    let conn = db.get_connection();

    let mut query = TournamentConfig::find();

    if let Some(current_seasonal) = query.current_seasonal {
        query = query.filter(tournament_config::Column::CurrentSeasonal.eq(current_seasonal));
    }

    if let Some(current_category) = query.current_category {
        query = query.filter(tournament_config::Column::CurrentCategory.eq(current_category));
    }

    if let Some(can_register) = query.can_register {
        query = query.filter(tournament_config::Column::CanRegister.eq(can_register));
    }

    let count = query.count(conn).await.map_err(|e| {
        tracing::error!("Failed to count tournament configs: {}", e);
        ApiResponse::internal_server_error("Failed to count tournament configs")
    })?;

    Ok(ApiResponse::success(count))
}
