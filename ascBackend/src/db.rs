use diesel::connection::SimpleConnection;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use std::env;

use crate::error::AppError;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// 启动迁移（schema 的单一事实源）：增量迁移放在 sql/ 文件里，在此登记后
/// 启动时自动执行一次（记录进 schema_migrations，不会重复执行）。
/// 新增迁移：在 sql/ 新建幂等脚本 + 在此登记一行。
///
/// dev_schema.sql 是全量基线：仅在空库自动执行；既有库会直接登记为已应用。
const MIGRATIONS: &[(&str, &str)] = &[
    ("dev_schema", include_str!("../sql/dev_schema.sql")),
    ("content_tables", include_str!("../sql/content_tables.sql")),
    ("bot_identity", include_str!("../sql/bot_identity.sql")),
    ("referee_tokens", include_str!("../sql/referee_tokens.sql")),
    ("player_stickers", include_str!("../sql/player_stickers.sql")),
    ("canvas_stickers", include_str!("../sql/canvas_stickers.sql")),
    ("add_schedule_season", include_str!("../sql/add_schedule_season.sql")),
    (
        "add_room_matchup_season",
        include_str!("../sql/add_room_matchup_season.sql"),
    ),
    (
        "add_cover_updated_at",
        include_str!("../sql/add_cover_updated_at.sql"),
    ),
    ("scheduled_tasks", include_str!("../sql/scheduled_tasks.sql")),
    ("staff_members", include_str!("../sql/staff_members.sql")),
];

pub fn init_pool() -> DbPool {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // 连接池大小：默认 20（r2d2 默认 10 对本项目偏小），可用 DB_POOL_SIZE 覆盖
    let max_size = env::var("DB_POOL_SIZE")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(20);

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(max_size)
        .build(manager)
        .expect("Failed to create db pool");
    run_migrations(&pool);
    pool
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    n: i64,
}

/// 启动时执行迁移。
///
/// - 每个迁移文件在一个事务里整体执行（原子），成功后记录进 schema_migrations；
/// - 单个迁移失败只记警告、不阻塞主服务启动（未记录的迁移下次启动自动重试）；
/// - 脚本仍建议保持幂等，便于手动 psql 重放排查。
fn run_migrations(pool: &DbPool) {
    let Ok(mut conn) = pool.get() else {
        tracing::warn!("无法获取数据库连接，跳过迁移");
        return;
    };
    if let Err(e) = diesel::sql_query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&mut conn)
    {
        tracing::error!("创建 schema_migrations 失败，跳过迁移: {e}");
        return;
    }

    for (name, sql) in MIGRATIONS {
        let name = *name;
        if migration_applied(&mut conn, name) {
            continue;
        }
        // 基线迁移只在空库执行；既有库（历史手工建表）直接登记为已应用
        if name == "dev_schema" && core_tables_exist(&mut conn) {
            tracing::info!("检测到既有数据库结构，登记基线 {name} 为已应用");
            record_migration(&mut conn, name);
            continue;
        }
        let result = conn.transaction::<(), diesel::result::Error, _>(|c| {
            c.batch_execute(sql)?;
            diesel::sql_query(
                "INSERT INTO schema_migrations (name) VALUES ($1) ON CONFLICT (name) DO NOTHING",
            )
            .bind::<diesel::sql_types::Text, _>(name)
            .execute(c)?;
            Ok(())
        });
        match result {
            Ok(()) => tracing::info!("迁移已应用: {name}"),
            Err(e) => tracing::warn!("迁移 {name} 执行失败（下次启动重试）: {e}"),
        }
    }
}

fn migration_applied(conn: &mut PgConnection, name: &str) -> bool {
    diesel::sql_query("SELECT count(*) AS n FROM schema_migrations WHERE name = $1")
        .bind::<diesel::sql_types::Text, _>(name)
        .get_result::<CountRow>(conn)
        .map(|r| r.n > 0)
        .unwrap_or(false)
}

fn record_migration(conn: &mut PgConnection, name: &str) {
    if let Err(e) = diesel::sql_query(
        "INSERT INTO schema_migrations (name) VALUES ($1) ON CONFLICT (name) DO NOTHING",
    )
    .bind::<diesel::sql_types::Text, _>(name)
    .execute(conn)
    {
        tracing::warn!("记录迁移 {name} 失败: {e}");
    }
}

/// 既有库检测：核心表 "User" 是否已存在（决定基线 dev_schema 是否执行）
fn core_tables_exist(conn: &mut PgConnection) -> bool {
    diesel::sql_query(
        "SELECT count(*) AS n FROM information_schema.tables WHERE table_name = $1",
    )
    .bind::<diesel::sql_types::Text, _>("User")
    .get_result::<CountRow>(conn)
    .map(|r| r.n > 0)
    .unwrap_or(false)
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
