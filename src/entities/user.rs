use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    #[sea_orm(unique)]
    pub osuid: i32,
    
    #[sea_orm(unique)]
    pub username: String,
    
    #[sea_orm(column_type = "Text", nullable)]
    pub avatar_url: Option<String>,
    
    #[sea_orm(column_type = "Text", nullable)]
    pub cover_url: Option<String>,
    
    pub country_code: String,
    
    #[sea_orm(column_type = "Double")]
    pub pp: f64,
    
    pub global_rank: i32,
    
    pub country_rank: i32,
    
    pub approved: bool,
    
    pub seed: i32,
    
    #[sea_orm(column_type = "String(Some(2))")]
    pub seasonal: Season,
    
    #[sea_orm(column_type = "String(Some(10))")]
    pub user_state: UserState,
    
    pub created_at: DateTime,
    
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "user_state")]
pub enum UserState {
    #[sea_orm(string_value = "ACTIVE")]
    Active,
    #[sea_orm(string_value = "REGISTERED")]
    Registered,
    #[sea_orm(string_value = "ABANDONED")]
    Abandoned,
    #[sea_orm(string_value = "BANNED")]
    Banned,
}

pub use super::tournament_config::Season;

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::team::Entity")]
    Teams,
    
    #[sea_orm(has_many = "super::message::Entity", via = "super::message::Relation::Sender")]
    SentMessages,
    
    #[sea_orm(has_many = "super::message::Entity", via = "super::message::Relation::Receiver")]
    ReceivedMessages,
    
    #[sea_orm(has_many = "super::beatmap::Entity")]
    Beatmaps,
    
    #[sea_orm(has_many = "super::beatmap_comment::Entity")]
    BeatmapComments,
    
    #[sea_orm(has_many = "super::multiplayer_solo_room::Entity", via = "super::multiplayer_solo_room::Relation::RedPlayer")]
    SoloRedRooms,
    
    #[sea_orm(has_many = "super::multiplayer_solo_room::Entity", via = "super::multiplayer_solo_room::Relation::BluePlayer")]
    SoloBlueRooms,
    
    #[sea_orm(has_many = "super::user_group::Entity")]
    UserGroups,
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teams.def()
    }
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SentMessages.def()
    }
}

impl Related<super::beatmap::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Beatmaps.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}