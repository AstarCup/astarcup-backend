use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMapSelectionRequest {
    pub beatmap_id: i32,
    pub beatmapset_id: i32,
    pub title: String,
    pub title_unicode: Option<String>,
    pub artist: String,
    pub artist_unicode: Option<String>,
    pub version: String,
    pub creator: String,
    pub star_rating: String,
    pub bpm: String,
    pub total_length: i32,
    pub max_combo: i32,
    pub ar: String,
    pub cs: String,
    pub od: String,
    pub hp: String,
    pub selected_mods: String,
    pub mod_position: i32,
    pub comment: Option<String>,
    pub selected_by: String,
    pub season: String,
    pub category: String,
    pub url: String,
    pub cover_url: Option<String>,
    pub approved: Option<i32>,
    pub padding: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMapSelectionRequest {
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub version: Option<String>,
    pub creator: Option<String>,
    pub star_rating: Option<String>,
    pub bpm: Option<String>,
    pub total_length: Option<i32>,
    pub max_combo: Option<i32>,
    pub ar: Option<String>,
    pub cs: Option<String>,
    pub od: Option<String>,
    pub hp: Option<String>,
    pub selected_mods: Option<String>,
    pub mod_position: Option<i32>,
    pub comment: Option<String>,
    pub season: Option<String>,
    pub category: Option<String>,
    pub url: Option<String>,
    pub cover_url: Option<String>,
    pub approved: Option<i32>,
    pub padding: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MapSelectionQueryParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub season: Option<String>,
    pub category: Option<String>,
    pub selected_by: Option<String>,
}
