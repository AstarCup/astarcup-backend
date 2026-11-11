use std::sync::Arc;

use crate::config::Config;
use crate::db::DbPool;
use crate::osu_client::OsuClient;
use crate::referee::RefereeService;

/// 全局应用状态
#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    pub osu: Arc<OsuClient>,
    /// 裁判控制台服务（osu! referee hub 连接管理）
    pub referee: Arc<RefereeService>,
}
