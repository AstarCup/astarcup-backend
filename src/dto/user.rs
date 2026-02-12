use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::entity::user::Model as UserModel;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateUserRequest {
    #[validate(length(min = 3, max = 50))]
    pub username: String,

    pub avatar_url: Option<String>,

    pub pp: Option<f32>,

    pub global_rank: Option<i32>,

    pub country: Option<String>,

    pub country_rank: Option<i32>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateUserRequest {
    #[validate(length(min = 3, max = 50))]
    pub username: Option<String>,

    pub avatar_url: Option<String>,

    pub pp: Option<f32>,

    pub global_rank: Option<i32>,

    pub country: Option<String>,

    pub country_rank: Option<i32>,

    pub approved: Option<i32>,

    pub season: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: i32,
    pub osu_id: String,
    pub username: String,
    pub registered_at: String,
    pub avatar_url: Option<String>,
    pub pp: Option<f32>,
    pub global_rank: Option<i32>,
    pub country: Option<String>,
    pub country_rank: Option<i32>,
    pub approved: Option<i32>,
    pub user_group: String,
    pub season: Option<i32>,
    pub registration_status: Option<String>,
    pub gu_server_user_id: Option<i32>,
    pub gu_server_username: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<UserModel> for UserResponse {
    fn from(user: UserModel) -> Self {
        Self {
            id: user.id,
            osu_id: user.osu_id,
            username: user.username,
            registered_at: user.registered_at.to_rfc3339(),
            avatar_url: user.avatar_url,
            pp: user.pp,
            global_rank: user.global_rank,
            country: user.country,
            country_rank: user.country_rank,
            approved: user.approved,
            user_group: format!("{:?}", user.user_group),
            season: user.season,
            registration_status: user
                .registration_status
                .map(|status| format!("{:?}", status)),
            gu_server_user_id: user.gu_server_user_id,
            gu_server_username: user.gu_server_username,
            created_at: user.created_at.to_rfc3339(),
            updated_at: user.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateUserGroupRequest {
    pub user_group: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateRegistrationStatusRequest {
    pub registration_status: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateSeasonRequest {
    pub season: i32,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateApprovedRequest {
    pub approved: i32,
}

#[derive(Debug, Deserialize, Validate, utoipa::IntoParams, utoipa::ToSchema)]
#[schema(description = "用户查询参数")]
pub struct UserQueryParams {
    pub username: Option<String>,
    pub country: Option<String>,
    pub user_group: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OsuUserData {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    pub statistics: Option<OsuStatistics>,
    pub country: Option<OsuCountry>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OsuStatistics {
    pub pp: Option<f32>,
    pub global_rank: Option<i32>,
    pub country_rank: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OsuCountry {
    pub code: String,
}
