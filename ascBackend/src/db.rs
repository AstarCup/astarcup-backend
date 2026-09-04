use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use std::env;

use crate::error::AppError;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn init_pool() -> DbPool {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create db pool")
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
