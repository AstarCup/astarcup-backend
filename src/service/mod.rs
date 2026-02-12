pub mod auth;
pub mod map_comments;
pub mod map_selections;
pub mod match_rooms;
pub mod match_schedules;
pub mod messages;
pub mod player_matchups;
pub mod tournament_settings;
pub mod user;

use sea_orm::DatabaseConnection;

/// 服务层共享状态
#[derive(Clone)]
pub struct ServiceState {
    pub db: DatabaseConnection,
}

impl ServiceState {
    /// 创建新的服务状态
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
