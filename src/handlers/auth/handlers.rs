use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    services::auth::{AuthService, OsuUserInfo},
    utils::{response::ok, AppError},
};

#[derive(Debug, Deserialize)]
pub struct AuthUrlQuery {
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthUrlResponse {
    pub success: bool,
    pub auth_url: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct AuthCallbackResponse {
    pub success: bool,
    pub token: String,
    pub user: OsuUserInfo,
}

pub async fn get_auth_url(
    State(auth_service): State<AuthService>,
    Query(query): Query<AuthUrlQuery>,
) -> Result<Json<AuthUrlResponse>, AppError> {
    // If a custom redirect_uri is provided, we could update the auth service
    // For now, we'll use the configured one
    let auth_url = auth_service.get_osu_auth_url();

    Ok(Json(AuthUrlResponse {
        success: true,
        auth_url,
    }))
}

pub async fn auth_callback(
    State(auth_service): State<AuthService>,
    Query(query): Query<AuthCallbackQuery>,
) -> Result<Json<AuthCallbackResponse>, AppError> {
    // Exchange code for token
    let token_data = auth_service.exchange_code_for_token(&query.code).await?;

    // Get user info from osu! API
    let user_info = auth_service
        .get_osu_user_info(&token_data.access_token)
        .await?;

    // Generate JWT token
    let jwt_token = auth_service.generate_jwt(&user_info)?;

    Ok(Json(AuthCallbackResponse {
        success: true,
        token: jwt_token,
        user: user_info,
    }))
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub success: bool,
    pub message: String,
}

pub async fn logout() -> Result<Json<LogoutResponse>, AppError> {
    // In a stateless JWT system, logout is handled on the client side
    // by removing the token. We could implement token blacklisting
    // if using Redis or similar.
    Ok(Json(LogoutResponse {
        success: true,
        message: "Logout successful. Please remove the token on the client side.".to_string(),
    }))
}

#[derive(Debug, Serialize)]
pub struct VerifyTokenResponse {
    pub success: bool,
    pub valid: bool,
    pub user_id: Option<String>,
    pub username: Option<String>,
}

pub async fn verify_token(
    State(auth_service): State<AuthService>,
    token: String,
) -> Result<Json<VerifyTokenResponse>, AppError> {
    match auth_service.verify_jwt(&token) {
        Ok(claims) => Ok(Json(VerifyTokenResponse {
            success: true,
            valid: true,
            user_id: Some(claims.sub),
            username: Some(claims.username),
        })),
        Err(_) => Ok(Json(VerifyTokenResponse {
            success: true,
            valid: false,
            user_id: None,
            username: None,
        })),
    }
}