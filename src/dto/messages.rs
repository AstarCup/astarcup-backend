use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMessageRequest {
    pub sender_osu_id: String,
    pub sender_username: String,
    pub receiver_osu_id: String,
    pub receiver_username: String,
    pub r#type: Option<String>,
    pub title: String,
    pub content: String,
    pub related_matchup_id: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMessageRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub status: Option<String>,
    pub response_action: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MessageQueryParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub sender_osu_id: Option<String>,
    pub receiver_osu_id: Option<String>,
    pub status: Option<String>,
}
