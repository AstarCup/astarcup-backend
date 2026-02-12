use std::env;

use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct Config {
    #[validate(length(min = 1))]
    pub database_url: String,

    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    pub host: String,

    #[validate(length(min = 1))]
    pub jwt_secret: String,

    pub jwt_expiry_hours: u64,

    pub osu_client_id: String,

    pub osu_client_secret: String,

    pub osu_redirect_uri: String,

    pub cors_origin: String,

    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Result<Self, crate::error::AppError> {
        dotenv::dotenv().ok();

        let config = Config {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| crate::error::AppError::ConfigError("DATABASE_URL未设置".into()))?,

            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|_| crate::error::AppError::ConfigError("PORT格式错误".into()))?,

            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),

            jwt_secret: env::var("JWT_SECRET")
                .map_err(|_| crate::error::AppError::ConfigError("JWT_SECRET未设置".into()))?,

            jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .map_err(|_| {
                    crate::error::AppError::ConfigError("JWT_EXPIRY_HOURS格式错误".into())
                })?,

            osu_client_id: env::var("OSU_CLIENT_ID")
                .map_err(|_| crate::error::AppError::ConfigError("OSU_CLIENT_ID未设置".into()))?,

            osu_client_secret: env::var("OSU_CLIENT_SECRET").map_err(|_| {
                crate::error::AppError::ConfigError("OSU_CLIENT_SECRET未设置".into())
            })?,

            osu_redirect_uri: env::var("OSU_REDIRECT_URI").map_err(|_| {
                crate::error::AppError::ConfigError("OSU_REDIRECT_URI未设置".into())
            })?,

            cors_origin: env::var("CORS_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),

            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
        };

        config
            .validate()
            .map_err(|e| crate::error::AppError::ConfigError(format!("配置验证失败: {}", e)))?;

        Ok(config)
    }
}
