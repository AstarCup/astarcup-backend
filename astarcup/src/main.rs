mod extensions;
mod middlewares;
mod routes;
mod utils;
mod entity;
mod error;

use sea_orm::{sqlx::database, Database, DatabaseConnection, DbErr};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let port = std::env::var("PORT")
        .ok()
        .map(|e| e.parse().ok())
        .flatten()
        .unwrap_or(8080);

    let host: std::net::IpAddr = std::env::var("HOST")
        .ok()
        .map(|e| e.parse().ok())
        .flatten()
        .unwrap_or("127.0.0.1".parse().unwrap());

    let app = routes::root::router::get_router().await;

    // Run app on local server
    let address = SocketAddr::from((host, port));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    print_banner();

    println!(
        "Server running on http://{}:{}",
        &address.ip(),
        &address.port()
    );
    // start server
    connect_database().await.unwrap();
    axum::serve(listener,app).await.unwrap();
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
async fn connect_database() -> Result<DatabaseConnection, DbErr> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://root:pass@localhost:5432/db".to_string());

    let database = Database::connect(&database_url).await?;
    Ok(database)
}
