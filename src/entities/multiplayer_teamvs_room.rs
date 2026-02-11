use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "multiplayer_teamvs_rooms")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub room_id: i32,
    
    #[sea_orm(unique)]
    pub team_red_id: i32,
    
    #[sea_orm(unique)]
    pub team_blue_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::TeamRedId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    RedTeam,
    
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::TeamBlueId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    BlueTeam,
    
    #[sea_orm(has_many = "super::multiplayer_room::Entity")]
    MultiplayerRooms,
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RedTeam.def()
    }
}

impl Related<super::multiplayer_room::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MultiplayerRooms.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}