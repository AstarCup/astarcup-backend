use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "tournament_configs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    pub tournament_name: String,
    
    #[sea_orm(column_type = "Double")]
    pub max_pp_for_registration: f64,
    
    #[sea_orm(column_type = "Double")]
    pub min_pp_for_registration: f64,
    
    #[sea_orm(column_type = "String(Some(2))")]
    pub current_seasonal: Season,
    
    #[sea_orm(column_type = "String(Some(4))")]
    pub current_category: Category,
    
    pub can_register: bool,
    
    pub created_at: DateTime,
    
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Season {
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    S10,
    Custom(String),
}

impl From<String> for Season {
    fn from(value: String) -> Self {
        match value.as_str() {
            "S1" => Season::S1,
            "S2" => Season::S2,
            "S3" => Season::S3,
            "S4" => Season::S4,
            "S5" => Season::S5,
            "S6" => Season::S6,
            "S7" => Season::S7,
            "S8" => Season::S8,
            "S9" => Season::S9,
            "S10" => Season::S10,
            _ => Season::Custom(value),
        }
    }
}

impl From<Season> for String {
    fn from(season: Season) -> Self {
        match season {
            Season::S1 => "S1".to_string(),
            Season::S2 => "S2".to_string(),
            Season::S3 => "S3".to_string(),
            Season::S4 => "S4".to_string(),
            Season::S5 => "S5".to_string(),
            Season::S6 => "S6".to_string(),
            Season::S7 => "S7".to_string(),
            Season::S8 => "S8".to_string(),
            Season::S9 => "S9".to_string(),
            Season::S10 => "S10".to_string(),
            Season::Custom(value) => value,
        }
    }
}

impl sea_orm::TryGetable for Season {
    fn try_get_by<I: sea_orm::ColIdx>(res: &sea_orm::QueryResult, index: I) -> Result<Self, sea_orm::TryGetError> {
        let value: String = res.try_get_by(index)?;
        Ok(Season::from(value))
    }
}

impl sea_orm::sea_query::ValueType for Season {
    fn try_from(v: sea_orm::Value) -> Result<Self, sea_orm::sea_query::ValueTypeErr> {
        match v {
            sea_orm::Value::String(Some(s)) => Ok(Season::from(s)),
            _ => Err(sea_orm::sea_query::ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "String".to_string()
    }

    fn column_type() -> sea_orm::sea_query::ColumnType {
        sea_orm::sea_query::ColumnType::String(Some(10))
    }

    fn array_type() -> sea_orm::sea_query::ArrayType {
        sea_orm::sea_query::ArrayType::String
    }
}

impl sea_orm::sea_query::Nullable for Season {
    fn null() -> sea_orm::Value {
        sea_orm::Value::String(None)
    }
}

impl sea_orm::sea_query::ValueTypeErr for Season {
    fn err() -> sea_orm::sea_query::ValueTypeErr {
        sea_orm::sea_query::ValueTypeErr
    }
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "category")]
pub enum Category {
    #[sea_orm(string_value = "QUA")]
    Qua,
    #[sea_orm(string_value = "RO16")]
    Ro16,
    #[sea_orm(string_value = "QF")]
    Qf,
    #[sea_orm(string_value = "SF")]
    Sf,
    #[sea_orm(string_value = "F")]
    F,
    #[sea_orm(string_value = "GF")]
    Gf,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}