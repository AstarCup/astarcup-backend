use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "teams")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    pub name: String,
    
    #[sea_orm(column_type = "Text")]
    pub banner_url: String,
    
    #[sea_orm(column_type = "String(Some(10))")]
    pub team_color: TeamColor,
    
    #[sea_orm(column_type = "String(Some(15))")]
    pub team_state: TeamState,
    
    pub created_at: DateTime,
    
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "team_color")]
pub enum TeamColor {
    #[sea_orm(string_value = "blue_team")]
    BlueTeam,
    #[sea_orm(string_value = "red_team")]
    RedTeam,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "team_state")]
pub enum TeamState {
    #[sea_orm(string_value = "ACTIVE")]
    Active,
    #[sea_orm(string_value = "UNAPPROVED")]
    Unapproved,
    #[sea_orm(string_value = "APPROVED")]
    Approved,
    #[sea_orm(string_value = "BANNED")]
    Banned,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user::Entity")]
    Members,
    
    #[sea_orm(has_many = "super::multiplayer_teamvs_room::Entity", via = "super::multiplayer_teamvs_room::Relation::RedTeam")]
    RedTeamRooms,
    
    #[sea_orm(has_many = "super::multiplayer_teamvs_room::Entity", via = "super::multiplayer_teamvs_room::Relation::BlueTeam")]
    BlueTeamRooms,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Members.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}