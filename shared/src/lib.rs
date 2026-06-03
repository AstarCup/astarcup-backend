use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// user
// --------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub osuid: i32,
    pub username: String,
    pub avatar_url: String,
    pub cover_id: String,
    pub cover_url: String,
    pub cover_custom_url: String,
    pub pp: f32,
    pub global_rank: i32,
    pub country_rank: i32,
    pub country_flag: String,
    pub user_group: UserGroup,
    pub create_at: DateTime<Utc>,
    pub update_at: DateTime<Utc>,
    pub registration_status: UserRegistrationStatus,
    pub registration_season: Vec<i32>,
    // 玩家五维
    pub user_accuracy: f32,
    pub user_stamina: f32,
    pub user_first_sight: f32,
    pub user_strategy: f32,
    pub user_experience: f32,
    pub user_custom_key: String,
    pub user_custom_key_value: f32,
}

// user请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLoginRequest {
    pub osuid: i32,
    // TODO: others
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegisterRequest {
    pub id: Uuid,
    pub osuid: i32,
    pub username: String,
    pub avatar_url: String,
    pub cover_id: Option<String>,
    pub cover_url: Option<String>,
    pub cover_custom_url: Option<String>,
    pub pp: f32,
    pub global_rank: i32,
    pub country_rank: i32,
    pub country_flag: String,
    pub user_group: UserGroup,
    pub create_at: DateTime<Utc>,
    pub registration_status: UserRegistrationStatus,
    pub registration_season: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdateOsuInfoRequest {
    pub id: Option<Uuid>,
    pub osuid: Option<i32>,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub cover_id: Option<String>,
    pub cover_url: Option<String>,
    pub cover_custom_url: Option<String>,
    pub pp: Option<f32>,
    pub global_rank: Option<i32>,
    pub country_rank: Option<i32>,
    pub country_flag: Option<String>,
    pub update_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdateGroupRequest {
    pub id: Option<Uuid>,
    pub osuid: Option<i32>,
    pub username: Option<String>,
    pub user_group: Option<UserGroup>,
    pub update_at: Option<DateTime<Utc>>,
}

// 五维的update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdateStatsRequest {
    pub id: Option<Uuid>,
    pub osuid: Option<i32>,
    pub username: Option<String>,
    pub user_accuracy: Option<f32>,
    pub user_stamina: Option<f32>,
    pub user_first_sight: Option<f32>,
    pub user_strategy: Option<f32>,
    pub user_experience: Option<f32>,
    pub user_custom_key: Option<String>,
    pub user_custom_key_value: Option<f32>,
    pub update_at: Option<DateTime<Utc>>,
}

// enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserGroup {
    None,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRegistrationStatus {
    NotRegistered,
    Registered,
}

// server settings
// ------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub id: Uuid,
    pub setting_key: String,
    pub setting_value: String,
    pub create_at: DateTime<Utc>,
    pub update_at: DateTime<Utc>,
}
// server setting request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettingCreateRequest {
    pub id: Uuid,
    pub setting_key: String,
    pub setting_value: String,
    pub create_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettingUpdateRequest {
    pub id: Option<Uuid>,
    pub setting_key: Option<String>,
    pub setting_value: Option<String>,
    pub update_at: Option<DateTime<Utc>>,
}

// mapool
// ------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapool {
    pub id: Uuid,
    pub beatmap_id: i32,
    pub beatmap_setid: i32,
    pub beatmap_url: String,
    pub title: String,
    pub title_unicode: String,
    pub artist: String,
    pub artist_unicode: String,
    pub cover_url: String,
    pub version: String,
    pub creator: String,
    pub star_rating: f32,
    pub bpm: f32,
    pub total_length: i32,
    pub max_combo: i32,
    pub ar: i32,
    pub cs: i32,
    pub od: i32,
    pub hp: i32,
    pub selected_mod_name: String,
    pub selected_mod_position: i32,
    pub comment: String,
    pub map_is_custom: bool,
    pub map_is_origin: bool,
    pub map_season: Vec<i32>,
    pub map_category: Vec<String>,
    pub map_status: MapoolMapStatus,
    pub create_at: DateTime<Utc>,
    pub update_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSelectCreateRequest {
    pub id: Option<Uuid>,
    pub beatmap_id: i32,
    pub beatmap_setid: i32,
    pub beatmap_url: String,
    pub title: String,
    pub title_unicode: String,
    pub artist: String,
    pub artist_unicode: String,
    pub cover_url: String,
    pub version: String,
    pub creator: String,
    pub star_rating: f32,
    pub bpm: f32,
    pub total_length: i32,
    pub max_combo: i32,
    pub ar: i32,
    pub cs: i32,
    pub od: i32,
    pub hp: i32,
    pub selected_mod_name: String,
    pub selected_mod_position: i32,
    pub comment: String,
    pub map_is_custom: bool,
    pub map_is_origin: bool,
    pub map_season: Vec<i32>,
    pub map_category: Vec<String>,
    pub map_status: MapoolMapStatus,
    pub create_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSelectUpdateRequest {
    pub id: Option<Uuid>,
    pub beatmap_id: Option<i32>,
    pub beatmap_setid: Option<i32>,
    pub beatmap_url: Option<String>,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub cover_url: Option<String>,
    pub version: Option<String>,
    pub creator: Option<String>,
    pub star_rating: Option<f32>,
    pub bpm: Option<f32>,
    pub total_length: Option<i32>,
    pub max_combo: Option<i32>,
    pub ar: Option<i32>,
    pub cs: Option<i32>,
    pub od: Option<i32>,
    pub hp: Option<i32>,
    pub selected_mod_name: Option<String>,
    pub selected_mod_position: Option<i32>,
    pub comment: Option<String>,
    pub map_is_custom: Option<bool>,
    pub map_is_origin: Option<bool>,
    pub map_season: Option<Vec<i32>>,
    pub map_category: Option<Vec<String>>,
    pub map_status: Option<MapoolMapStatus>,
    pub update_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSelectDeleteRequest {
    pub id: Option<Uuid>,
    pub beatmap_id: Option<i32>,
    pub map_status: Option<MapoolMapStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapoolMapStatus {
    None,
    Verify,
    Approved,
    Deleted,
}

// Map Comment
// ---------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapoolComment {
    pub id: Uuid,
    pub mapool_id: Uuid,
    pub user_id: Uuid,
    pub comment: String,
    pub create_at: DateTime<Utc>,
    pub update_at: DateTime<Utc>,
}
// map comment request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapoolCommentCreateRequest {
    pub id: Option<Uuid>,
    pub mapool_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub comment: Option<String>,
    pub create_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapoolCommentUpdateRequest {
    pub id: Option<Uuid>,
    pub mapool_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub comment: Option<String>,
    pub update_at: Option<DateTime<Utc>>,
}

// PlayerMatchup
// ------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMatchup {
    pub id: Uuid,
    pub player_red: Vec<User>,
    pub player_blue: Vec<User>,
    pub match_status: PlayerMatchStatus,
    pub create_at: DateTime<Utc>,
    pub update_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerMatchStatus {
    Open,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMatchupCreateRequest {
    pub id: Option<Uuid>,
    pub player_red: Option<Vec<User>>,
    pub player_blue: Option<Vec<User>>,
    pub match_status: Option<PlayerMatchStatus>,
    pub create_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMatchupUpdateRequest {
    pub id: Option<Uuid>,
    pub player_red: Option<Vec<User>>,
    pub player_blue: Option<Vec<User>>,
    pub match_status: Option<PlayerMatchStatus>,
    pub update_at: Option<DateTime<Utc>>,
}
