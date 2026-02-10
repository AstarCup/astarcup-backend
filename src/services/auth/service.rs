use crate::config::Settings;
use crate::utils::AppError;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct OsuUserInfo {
    pub id: u64,
    pub username: String,
    pub avatar_url: String,
    pub cover_url: String,
    pub country_code: String,
    pub statistics: Option<OsuStatistics>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OsuStatistics {
    pub pp: f64,
    pub global_rank: Option<u64>,
    pub country_rank: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OsuTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String, // osu! user ID
    pub username: String,
    pub exp: usize, // Expiration time
    pub iat: usize, // Issued at
}

pub struct AuthService {
    client: Client,
    settings: Settings,
}

impl AuthService {
    pub fn new(settings: Settings) -> Self {
        Self {
            client: Client::new(),
            settings,
        }
    }

    pub fn get_osu_auth_url(&self) -> String {
        let params = vec![
            ("client_id", self.settings.oauth.client_id.as_str()),
            ("redirect_uri", self.settings.oauth.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", "identify public"),
        ];

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        format!("https://osu.ppy.sh/oauth/authorize?{}", query_string)
    }

    pub async fn exchange_code_for_token(&self, code: &str) -> Result<OsuTokenResponse, AppError> {
        let params = [
            ("client_id", self.settings.oauth.client_id.as_str()),
            ("client_secret", self.settings.oauth.client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", self.settings.oauth.redirect_uri.as_str()),
        ];

        let response = self
            .client
            .post("https://osu.ppy.sh/oauth/token")
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(AppError::Auth(format!(
                "Failed to exchange code for token: {}",
                error_text
            )));
        }

        let token_data: OsuTokenResponse = response.json().await?;
        Ok(token_data)
    }

    pub async fn get_osu_user_info(&self, access_token: &str) -> Result<OsuUserInfo, AppError> {
        let response = self
            .client
            .get("https://osu.ppy.sh/api/v2/me")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(AppError::Auth(format!(
                "Failed to get user info: {}",
                error_text
            )));
        }

        let user_info: OsuUserInfo = response.json().await?;
        Ok(user_info)
    }

    pub fn generate_jwt(&self, user_info: &OsuUserInfo) -> Result<String, AppError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let expiry = now + (self.settings.jwt.expiry_hours as usize * 3600);

        let claims = JwtClaims {
            sub: user_info.id.to_string(),
            username: user_info.username.clone(),
            exp: expiry,
            iat: now,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.jwt.secret.as_bytes()),
        )?;

        Ok(token)
    }

    pub fn verify_jwt(&self, token: &str) -> Result<JwtClaims, AppError> {
        let token_data = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(self.settings.jwt.secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }
}