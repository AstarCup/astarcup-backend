use axum::extract::FromRequestParts;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: i32,
    pub osu_id: String,
    pub username: String,
    pub user_group: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync + Clone + 'static,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = std::result::Result<Self, Self::Rejection>> + Send {
        async move {
            let auth_header = parts
                .headers
                .get("Authorization")
                .and_then(|header| header.to_str().ok())
                .ok_or_else(|| AppError::Unauthorized("Authorization header missing".into()))?;

            if !auth_header.starts_with("Bearer ") {
                return Err(AppError::Unauthorized(
                    "Invalid authorization format".into(),
                ));
            }

            let token = &auth_header[7..];

            let claims = crate::utils::jwt::verify_token(token)
                .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))?;

            Ok(AuthUser {
                id: 0,
                osu_id: claims.sub,
                username: String::new(),
                user_group: claims.user_group,
            })
        }
    }
}

pub fn require_admin(user: &AuthUser) -> std::result::Result<(), AppError> {
    if user.user_group != "admin" {
        return Err(AppError::Forbidden("只有管理员才能执行此操作".into()));
    }
    Ok(())
}
