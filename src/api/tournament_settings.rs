use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, put},
};

use crate::{
    dto::{
        ApiResponse,
        tournament_settings::{
            TournamentSettingQueryParams, TournamentSettingResponse, UpdateTournamentSettingRequest,
        },
    },
    error::Result,
    middleware::auth::{AuthUser, require_admin},
    service::{ServiceState, tournament_settings::TournamentSettingService},
};

pub fn routes() -> Router<ServiceState> {
    Router::new()
        .route("/", get(get_tournament_settings))
        .route("/", put(update_tournament_settings))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PaginatedTournamentSettingsResponse {
    pub items: Vec<TournamentSettingResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

#[utoipa::path(
    get,
    path = "/api/tournament-settings",
    params(TournamentSettingQueryParams),
    responses(
        (status = 200, description = "获取比赛设置列表成功", body = PaginatedTournamentSettingsResponse),
        (status = 401, description = "未授权"),
        (status = 500, description = "内部服务器错误"),
    ),
    tag = "tournament-settings"
)]
async fn get_tournament_settings(
    State(state): State<ServiceState>,
    Query(params): Query<TournamentSettingQueryParams>,
) -> Result<Json<ApiResponse<PaginatedTournamentSettingsResponse>>> {
    let service = TournamentSettingService::new(state.db);
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);
    let (settings, total) = service.find_all(params).await?;

    let response = PaginatedTournamentSettingsResponse {
        items: settings
            .into_iter()
            .map(|s| TournamentSettingResponse {
                id: s.id,
                tournament_name: s.tournament_name,
                max_pp_for_registration: s.max_pp_for_registration,
                min_pp_for_registration: s.min_pp_for_registration,
                current_season: s.current_season,
                current_season_stage: s.current_season_stage,
                mappool_visible: s.mappool_visible,
                created_at: s.created_at.to_rfc3339(),
                updated_at: s.updated_at.to_rfc3339(),
            })
            .collect(),
        total,
        page,
        page_size,
        total_pages: (total as f64 / page_size as f64).ceil() as u64,
    };

    Ok(Json(ApiResponse::success(response)))
}

#[utoipa::path(
    put,
    path = "/api/tournament-settings",
    request_body = UpdateTournamentSettingRequest,
    responses(
        (status = 200, description = "更新比赛设置成功", body = TournamentSettingResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足，需要管理员权限"),
        (status = 500, description = "内部服务器错误"),
    ),
    tag = "tournament-settings"
)]
async fn update_tournament_settings(
    State(state): State<ServiceState>,
    auth_user: AuthUser,
    Json(data): Json<UpdateTournamentSettingRequest>,
) -> Result<Json<ApiResponse<TournamentSettingResponse>>> {
    // 验证用户是否为admin
    require_admin(&auth_user)?;

    let service = TournamentSettingService::new(state.db);

    // 检查是否存在设置，如果不存在则初始化
    let settings = match service.get_current_settings().await? {
        Some(existing) => {
            // 更新现有设置
            service.update(existing.id, data).await?
        }
        None => {
            // 初始化新设置
            let create_data = crate::dto::tournament_settings::CreateTournamentSettingRequest {
                tournament_name: data
                    .tournament_name
                    .unwrap_or_else(|| "Default Tournament".to_string()),
                max_pp_for_registration: data.max_pp_for_registration,
                min_pp_for_registration: data.min_pp_for_registration,
                current_season: data.current_season,
                current_season_stage: data.current_season_stage,
                mappool_visible: data.mappool_visible,
            };
            service.create(create_data).await?
        }
    };

    let response = TournamentSettingResponse {
        id: settings.id,
        tournament_name: settings.tournament_name,
        max_pp_for_registration: settings.max_pp_for_registration,
        min_pp_for_registration: settings.min_pp_for_registration,
        current_season: settings.current_season,
        current_season_stage: settings.current_season_stage,
        mappool_visible: settings.mappool_visible,
        created_at: settings.created_at.to_rfc3339(),
        updated_at: settings.updated_at.to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(response)))
}
