use sea_orm::{Database as SeaDatabase, DatabaseConnection, DbErr};
use tracing::info;

pub struct Database {
    pub connection: DatabaseConnection,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, DbErr> {
        info!("Connecting to database: {}", database_url);
        
        let connection = SeaDatabase::connect(database_url).await?;
        
        info!("Database connection established successfully");
        
        Ok(Self { connection })
    }

    pub fn get_connection(&self) -> &DatabaseConnection {
        &self.connection
    }

    pub async fn test_connection(&self) -> Result<(), DbErr> {
        info!("Testing database connection...");
        
        // Simple query to test connection
        let _: Option<(i32,)> = sea_orm::QuerySelect::query(
            &mut sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::MySql,
                "SELECT 1".to_string(),
            ),
        )
        .one(&self.connection)
        .await?;
        
        info!("Database connection test passed");
        Ok(())
    }
}