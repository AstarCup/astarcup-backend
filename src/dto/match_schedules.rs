use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMatchScheduleRequest {
    pub room_id: i32,
    pub player1_osu_id: String,
    pub player1_username: String,
    pub player2_osu_id: String,
    pub player2_username: String,
    pub created_by: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMatchScheduleRequest {
    pub red_score: Option<i32>,
    pub blue_score: Option<i32>,
    pub status: Option<String>,
    pub replay_link: Option<String>,
    pub match_link: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MatchScheduleQueryParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub room_id: Option<i32>,
    pub status: Option<String>,
    pub player_osu_id: Option<String>,
}
