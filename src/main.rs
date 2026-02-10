mod config;
mod database;
mod handlers;
mod routes;
mod services;
mod utils;

use axum::{
    http::{HeaderValue, Method},
    Router,
};
use config::Settings;
use database::Database;
use routes::create_router;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting AstraCup backend server...");

    // Load configuration
    let settings = Settings::from_env()?;
    info!("Configuration loaded successfully");

    // Initialize database connection
    let database = Database::new(settings.database_url()).await?;
    database.test_connection().await?;
    info!("Database connection established");

    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_origin(settings.cors.origin.parse::<HeaderValue>()?)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
        .allow_headers(tower_http::cors::Any);

    // Create application router
    let app = create_router(database, settings.clone())
        .layer(cors);

    // Start server
    let addr: SocketAddr = settings.server_address().parse()?;
    info!("Server listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}