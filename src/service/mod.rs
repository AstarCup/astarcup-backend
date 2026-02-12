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

/// 确保 ServiceState 实现了 Send + Sync traits
unsafe impl Send for ServiceState {}
unsafe impl Sync for ServiceState {}

impl ServiceState {
    /// 创建新的服务状态
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
