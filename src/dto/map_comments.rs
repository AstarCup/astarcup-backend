use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMapCommentRequest {
    pub map_selection_id: i32,
    pub osu_id: String,
    pub username: String,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMapCommentRequest {
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MapCommentQueryParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub map_selection_id: Option<i32>,
    pub osu_id: Option<String>,
}
