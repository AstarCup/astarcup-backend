use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "multiplayer_solo_rooms")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub room_id: i32,
    
    #[sea_orm(unique)]
    pub player_red_id: i32,
    
    #[sea_orm(unique)]
    pub player_blue_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::PlayerRedId",
        to = "super::user::Column::Osuid",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    RedPlayer,
    
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::PlayerBlueId",
        to = "super::user::Column::Osuid",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    BluePlayer,
    
    #[sea_orm(has_many = "super::multiplayer_room::Entity")]
    MultiplayerRooms,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RedPlayer.def()
    }
}

impl Related<super::multiplayer_room::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MultiplayerRooms.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}