use std::sync::Arc;

use axum::Router;
use config::Config;
use sea_orm::Database;
use service::ServiceState;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod config;
mod dto;
mod entity;
mod error;
mod middleware;
mod service;
mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 加载配置
    let config = Config::from_env()?;

    print_banner();
    println!("启动 AstraCup 后端服务...");

    println!("连接数据库...");
    let db = Database::connect(&config.database_url).await?;
    println!("数据库连接成功");

    // 创建服务状态
    let service_state = ServiceState::new(db);

    // 配置应用
    let app = api::configure_routes().with_state(service_state);

    println!("配置加载成功");

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
        .await
        .unwrap();

    println!("服务器启动成功");
    println!("监听地址: {}:{}", config.host, config.port);
    println!("数据库: {}", mask_database_url(&config.database_url));
    println!("API文档: http://{}:{}/api/docs", config.host, config.port);
    println!("健康检查: http://{}:{}/health", config.host, config.port);

    axum::serve(listener, app).await?;

    Ok(())
}

fn print_banner() {
    println!(r"     _        _              ____            ");
    println!(r"    / \   ___| |_ __ _ _ __ / ____   _ _ __  ");
    println!(r"   / _ \ / __| __/ _` | '__| |  | | | | '_ \ ");
    println!(r"  / ___ \\__ \ || (_| | |  | |__| |_| | |_) |");
    println!(r" /_/   \_\___/\__\__,_|_|   \____\__,_| .__/ ");
    println!(r"                                      |_|    ");
    println!(r"==============================================");
}

fn mask_database_url(url: &str) -> String {
    if let Some(pos) = url.find('@') {
        let mut masked = url.to_string();
        if let Some(start) = url.find("://") {
            let end = pos;
            if end > start + 3 {
                let credentials = &url[start + 3..end];
                if let Some(colon_pos) = credentials.find(':') {
                    let username = &credentials[..colon_pos];
                    masked.replace_range(start + 3..end, &format!("{}:******", username));
                }
            }
        }
        masked
    } else {
        url.to_string()
    }
}
