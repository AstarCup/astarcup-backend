use sea_orm::prelude::{Date, Time};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMatchRoomRequest {
    pub room_name: String,
    pub round_number: i32,
    #[schema(value_type = String)]
    pub match_date: Date,
    #[schema(value_type = String)]
    pub match_time: Time,
    pub match_number: i32,
    pub max_participants: Option<i32>,
    pub created_by: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMatchRoomRequest {
    pub room_name: Option<String>,
    pub round_number: Option<i32>,
    #[schema(value_type = Option<String>)]
    pub match_date: Option<Date>,
    #[schema(value_type = Option<String>)]
    pub match_time: Option<Time>,
    pub match_number: Option<i32>,
    pub max_participants: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MatchRoomQueryParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub round_number: Option<i32>,
    pub status: Option<String>,
}
