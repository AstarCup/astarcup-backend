use sea_orm::prelude::DateTime;
use serde::{Deserialize, Serialize};
use crate::entity::sea_orm_active_enums::{UserGroup, UserState};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct User {
    pub id: i32,
    pub osu_id: i32,
    pub username: String,
    pub avatar_url: String,
    pub cover_url: String,
    pub pp: Option<i32>,
    pub global_rank: Option<i32>,
    pub country_code: Option<String>,
    pub country_rank: Option<i32>,
    pub user_state: UserState,
    pub user_group: UserGroup,
    pub season: Option<i32>,
    pub gu_server_user_id: Option<i32>,
    pub gu_server_username: Option<String>,
    pub gu_server_bind_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub osu_refresh_token: Option<String>,
    pub gu_refresh_token: Option<String>,
}
