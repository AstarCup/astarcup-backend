use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "multiplayer_rooms")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub room_id: i32,
    
    #[sea_orm(column_type = "String(Some(10))")]
    pub r#type: MultiplayerRoomType,
    
    #[sea_orm(column_type = "String(Some(2))")]
    pub season: Season,
    
    #[sea_orm(column_type = "String(Some(4))")]
    pub category: Category,
    
    pub multiplayer_solo_room_room_id: Option<i32>,
    
    pub multiplayer_teamvs_room_room_id: Option<i32>,
    
    pub score_red: i32,
    
    pub score_blue: i32,
    
    pub start_time: DateTime,
    
    pub end_time: DateTime,
    
    pub created_at: DateTime,
    
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "multiplayer_room_type")]
pub enum MultiplayerRoomType {
    #[sea_orm(string_value = "SOLO")]
    Solo,
    #[sea_orm(string_value = "TEAM_VS")]
    TeamVs,
}

// 使用与tournament_config相同的Season和Category枚举
pub use super::tournament_config::{Season, Category};

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::multiplayer_solo_room::Entity",
        from = "Column::MultiplayerSoloRoomRoomId",
        to = "super::multiplayer_solo_room::Column::RoomId",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    MultiplayerSoloRoom,
    
    #[sea_orm(
        belongs_to = "super::multiplayer_teamvs_room::Entity",
        from = "Column::MultiplayerTeamvsRoomRoomId",
        to = "super::multiplayer_teamvs_room::Column::RoomId",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    MultiplayerTeamvsRoom,
    
    #[sea_orm(has_many = "super::message::Entity")]
    Messages,
}

impl Related<super::multiplayer_solo_room::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MultiplayerSoloRoom.def()
    }
}

impl Related<super::multiplayer_teamvs_room::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MultiplayerTeamvsRoom.def()
    }
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Messages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}