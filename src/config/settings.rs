use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry_hours: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub log_level: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CorsConfig {
    pub origin: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseConfig,
    pub oauth: OAuthConfig,
    pub jwt: JwtConfig,
    pub server: ServerConfig,
    pub cors: CorsConfig,
    pub redis: RedisConfig,
}

impl Settings {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv::dotenv().ok();

        let settings = Settings {
            database: DatabaseConfig {
                url: std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "mysql://root:@localhost:3306/astracup".to_string()),
            },
            oauth: OAuthConfig {
                client_id: std::env::var("OSU_CLIENT_ID").unwrap_or_default(),
                client_secret: std::env::var("OSU_CLIENT_SECRET").unwrap_or_default(),
                redirect_uri: std::env::var("OSU_REDIRECT_URI")
                    .unwrap_or_else(|_| "http://localhost:3000/api/auth/callback/osu".to_string()),
            },
            jwt: JwtConfig {
                secret: std::env::var("JWT_SECRET")
                    .unwrap_or_else(|_| "default_secret_change_in_production".to_string()),
                expiry_hours: std::env::var("JWT_EXPIRY_HOURS")
                    .unwrap_or_else(|_| "24".to_string())
                    .parse()
                    .unwrap_or(24),
            },
            server: ServerConfig {
                port: std::env::var("PORT")
                    .unwrap_or_else(|_| "3001".to_string())
                    .parse()
                    .unwrap_or(3001),
                host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            },
            cors: CorsConfig {
                origin: std::env::var("CORS_ORIGIN")
                    .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            },
            redis: RedisConfig {
                url: std::env::var("REDIS_URL").ok(),
            },
        };

        Ok(settings)
    }

    pub fn database_url(&self) -> &str {
        &self.database.url
    }

    pub fn jwt_secret(&self) -> &str {
        &self.jwt.secret
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}