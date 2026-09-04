// Astra Cup 后端（Rust / axum 重写版）
// 统一 JWT 认证 + 整合优化后的 API（见各 routes 模块顶部注释）
use std::sync::Arc;

use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod auth;
mod config;
mod db;
mod enums;
mod error;
mod models;
mod osu_client;
mod routes;
mod schema;
mod state;
mod util;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "asc_backend=info,tower_http=info".into()),
        )
        .init();

    let config = Arc::new(Config::from_env());
    let pool = db::init_pool();

    let http = reqwest::Client::builder()
        .user_agent("astarcup-backend/1.0")
        .build()
        .expect("构建 HTTP 客户端失败");
    let osu = Arc::new(osu_client::OsuClient::new(http.clone(), &config));

    let cors = CorsLayer::new()
        .allow_origin(
            config
                .cors_origins
                .iter()
                .map(|o| o.parse().unwrap())
                .collect::<Vec<_>>(),
        )
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
        ])
        .allow_credentials(true);

    let state = AppState {
        pool,
        config: config.clone(),
        http,
        osu,
    };
    let app = routes::build_router()
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("端口绑定失败");
    tracing::info!("Astra Cup 后端已启动: http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
