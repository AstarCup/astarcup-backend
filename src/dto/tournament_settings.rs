use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTournamentSettingRequest {
    pub tournament_name: String,
    pub max_pp_for_registration: Option<f32>,
    pub min_pp_for_registration: Option<f32>,
    pub current_season: Option<String>,
    pub current_season_stage: Option<String>,
    pub mappool_visible: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTournamentSettingRequest {
    pub tournament_name: Option<String>,
    pub max_pp_for_registration: Option<f32>,
    pub min_pp_for_registration: Option<f32>,
    pub current_season: Option<String>,
    pub current_season_stage: Option<String>,
    pub mappool_visible: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct TournamentSettingQueryParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub current_season: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TournamentSettingResponse {
    pub id: i32,
    pub tournament_name: String,
    pub max_pp_for_registration: Option<f32>,
    pub min_pp_for_registration: Option<f32>,
    pub current_season: Option<String>,
    pub current_season_stage: Option<String>,
    pub mappool_visible: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}
