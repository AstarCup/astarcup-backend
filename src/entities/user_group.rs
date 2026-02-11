use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_groups")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    pub user_id: i32,
    
    #[sea_orm(column_type = "String(Some(15))")]
    pub group_type: UserGroupType,
    
    pub created_at: DateTime,
    
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "user_group_type")]
pub enum UserGroupType {
    #[sea_orm(string_value = "HOST")]
    Host,
    #[sea_orm(string_value = "ADMIN")]
    Admin,
    #[sea_orm(string_value = "POOLER")]
    Pooler,
    #[sea_orm(string_value = "STREAMER")]
    Streamer,
    #[sea_orm(string_value = "TESTER")]
    Tester,
    #[sea_orm(string_value = "GFX")]
    Gfx,
    #[sea_orm(string_value = "SHEETER")]
    Sheeter,
    #[sea_orm(string_value = "COMMENTATOR")]
    Commentator,
    #[sea_orm(string_value = "MAPPER")]
    Mapper,
    #[sea_orm(string_value = "REFEREE")]
    Referee,
    #[sea_orm(string_value = "PLAYER")]
    Player,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Osuid",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}