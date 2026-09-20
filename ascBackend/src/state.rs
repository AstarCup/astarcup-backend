use std::sync::Arc;

use crate::config::Config;
use crate::db::DbPool;
use crate::osu_client::OsuClient;

/// 全局应用状态
#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    pub osu: Arc<OsuClient>,
}
