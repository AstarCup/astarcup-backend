use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    pub title: String,
    
    #[sea_orm(column_type = "Text")]
    pub content: String,
    
    pub sender_id: i32,
    
    pub receiver_id: i32,
    
    pub room_id: Option<i32>,
    
    pub is_read: bool,
    
    pub is_deleted: bool,
    
    pub is_system_message: bool,
    
    pub is_accepted: bool,
    
    pub created_at: DateTime,
    
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::SenderId",
        to = "super::user::Column::Osuid",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    Sender,
    
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::ReceiverId",
        to = "super::user::Column::Osuid",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    Receiver,
    
    #[sea_orm(
        belongs_to = "super::multiplayer_room::Entity",
        from = "Column::RoomId",
        to = "super::multiplayer_room::Column::RoomId",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    Room,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sender.def()
    }
}

impl Related<super::multiplayer_room::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Room.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}