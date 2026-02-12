use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePlayerMatchupRequest {
    pub player1_osu_id: String,
    pub player1_username: String,
    pub player2_osu_id: String,
    pub player2_username: String,
    pub created_by: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePlayerMatchupRequest {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PlayerMatchupQueryParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub status: Option<String>,
    pub player_osu_id: Option<String>,
}
