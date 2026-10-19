use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use std::env;

use crate::error::AppError;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn init_pool() -> DbPool {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create db pool");
    ensure_schema(&pool);
    pool
}

/// 项目暂无 migration 框架，启动时幂等补齐 Bot 表和赛程赛季字段。
/// 失败只记录警告，不阻塞主服务启动（其余接口仍可使用）。
fn ensure_schema(pool: &DbPool) {
    const DDL: &[&str] = &[
        "CREATE TABLE IF NOT EXISTS bot_platform_bindings (
            id SERIAL PRIMARY KEY,
            platform VARCHAR(64) NOT NULL,
            platform_user_id VARCHAR(128) NOT NULL,
            osu_id VARCHAR(64) NOT NULL,
            display_name VARCHAR(128),
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE UNIQUE INDEX IF NOT EXISTS bot_platform_bindings_platform_user_idx
            ON bot_platform_bindings (platform, platform_user_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS bot_platform_bindings_platform_osu_idx
            ON bot_platform_bindings (platform, osu_id)",
        "CREATE INDEX IF NOT EXISTS bot_platform_bindings_osu_idx
            ON bot_platform_bindings (osu_id)",
        "CREATE TABLE IF NOT EXISTS bot_bind_codes (
            id SERIAL PRIMARY KEY,
            code_hash VARCHAR(64) NOT NULL UNIQUE,
            osu_id VARCHAR(64) NOT NULL,
            expires_at TIMESTAMP NOT NULL,
            used_at TIMESTAMP,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE INDEX IF NOT EXISTS bot_bind_codes_osu_idx
            ON bot_bind_codes (osu_id)",
        "CREATE TABLE IF NOT EXISTS bot_bind_links (
            id SERIAL PRIMARY KEY,
            token_hash VARCHAR(64) NOT NULL UNIQUE,
            platform VARCHAR(64) NOT NULL,
            platform_user_id VARCHAR(128) NOT NULL,
            display_name VARCHAR(128),
            expires_at TIMESTAMP NOT NULL,
            used_at TIMESTAMP,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE INDEX IF NOT EXISTS bot_bind_links_identity_idx
            ON bot_bind_links (platform, platform_user_id)",
        "ALTER TABLE \"MatchSchedule\" ADD COLUMN IF NOT EXISTS \"season\" TEXT",
        "CREATE INDEX IF NOT EXISTS \"MatchSchedule_season_idx\"
            ON \"MatchSchedule\" (\"season\")",
    ];

    let Ok(mut conn) = pool.get() else {
        tracing::warn!("无法获取数据库连接，跳过 schema 初始化");
        return;
    };
    for stmt in DDL {
        if let Err(e) = diesel::sql_query(*stmt).execute(&mut conn) {
            tracing::warn!("schema 初始化失败: {e}");
        }
    }
}

/// 在阻塞线程池中执行数据库操作，避免阻塞异步运行时。
/// 闭包内可用 `?` 返回 diesel 错误（自动转换）或自定义 AppError。
pub async fn run_db<F, T>(pool: &DbPool, f: F) -> Result<T, AppError>
where
    F: FnOnce(&mut PgConnection) -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    let pool = pool.clone();
    let res = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get()?;
        f(&mut conn)
    })
    .await
    .map_err(|e| AppError::internal(format!("任务调度失败: {e}")))?;
    res
}
