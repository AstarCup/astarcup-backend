// Astra Cup 后端（Rust / axum 重写版）
// 统一 JWT 认证 + 整合优化后的 API（见各 routes 模块顶部注释）
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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

// 全局请求日志：记录 IP、方法、路径、状态码、耗时与登录用户名
async fn log_request(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let ip = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .unwrap_or_else(|| "-".to_string());
    let username = req
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|raw| {
            raw.split(';')
                .map(|p| p.trim())
                .find_map(|p| p.strip_prefix(&format!("{}=", auth::SESSION_COOKIE)))
        })
        .and_then(|token| auth::verify_token(&state.config, token).ok())
        .map(|claims| claims.username)
        .unwrap_or_else(|| "-".to_string());
    let response = next.run(req).await;
    tracing::info!(
        "[req] {} {} {} {} -> {} ({}ms)",
        ip,
        username,
        method,
        uri,
        response.status(),
        start.elapsed().as_millis()
    );
    response
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // 日志：stdout 彩色输出 + 落盘（按天滚动，控制单文件体积）
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "ascBackend=info,tower_http=info".into());
    let file_appender = tracing_appender::rolling::daily("logs", "ascBackend.log");
    let (file_writer, _file_guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false),
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

    // 后台任务：每分钟把距开赛不足 2 小时的房间自动置为 closed
    {
        let bg_state = state.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                ticker.tick().await;
                match routes::rooms_routes::close_due_rooms(&bg_state).await {
                    Ok(n) if n > 0 => tracing::info!("自动关闭 {n} 个到点房间"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("自动关闭房间失败: {}", e.message),
                }
            }
        });
    }

    let app = routes::build_router()
        .layer(cors)
        .layer(axum::middleware::from_fn_with_state(state.clone(), log_request))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("端口绑定失败");
    tracing::info!("Astra Cup 后端已启动: http://{addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}
