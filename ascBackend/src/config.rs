use std::env;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    #[allow(dead_code)]
    pub database_url: String,
    pub cors_origins: Vec<String>,
    pub frontend_url: String,
    pub jwt_secret: String,
    pub jwt_expire_days: i64,
    pub cookie_domain: String,
    pub cookie_secure: bool,
    pub osu_client_id: String,
    pub osu_client_secret: String,
    pub osu_redirect_uri: String,
    pub storage_dir: String,
    pub vocab_dir: String,
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            port: env_or("PORT", "4000").parse().unwrap_or(4000),
            database_url: env_or("DATABASE_URL", ""),
            cors_origins: env_or("CORS_ORIGINS", "http://localhost:3000")
                .split(',')
                .map(|s| s.trim().trim_end_matches('/').to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            frontend_url: env_or("FRONTEND_URL", "http://localhost:3000"),
            jwt_secret: env_or("JWT_SECRET", "asc-dev-secret"),
            jwt_expire_days: env_or("JWT_EXPIRE_DAYS", "30").parse().unwrap_or(30),
            cookie_domain: env_or("COOKIE_DOMAIN", ""),
            cookie_secure: env_or("COOKIE_SECURE", "false") == "true",
            osu_client_id: env_or("OSU_CLIENT_ID", ""),
            osu_client_secret: env_or("OSU_CLIENT_SECRET", ""),
            osu_redirect_uri: env_or(
                "OSU_REDIRECT_URI",
                "http://localhost:4000/api/auth/callback",
            ),
            storage_dir: env_or("STORAGE_DIR", "./storage"),
            vocab_dir: env_or(
                "VOCAB_DIR",
                "/home/aecw/astarcup/astracup-web/src/app/api/content-detect/Vocabulary",
            ),
        }
    }
}
