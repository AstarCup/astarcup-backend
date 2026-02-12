use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sea_orm::DbErr;
use serde_json::json;
use thiserror::Error;

/// 应用程序错误枚举
#[derive(Error, Debug)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    DatabaseError(#[from] DbErr),

    #[error("未找到资源: {0}")]
    NotFound(String),

    #[error("认证失败: {0}")]
    Unauthorized(String),

    #[error("权限不足: {0}")]
    Forbidden(String),

    #[error("验证失败: {0}")]
    ValidationError(String),

    #[error("业务错误: {0}")]
    BusinessError(String),

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("外部API错误: {0}")]
    ExternalApiError(String),

    #[error("内部服务器错误: {0}")]
    InternalError(String),
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AppError::BusinessError(_) => StatusCode::BAD_REQUEST,
            AppError::ConfigError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::ExternalApiError(_) => StatusCode::BAD_GATEWAY,
            AppError::DatabaseError(_) | AppError::InternalError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    pub fn error_message(&self) -> String {
        match self {
            AppError::DatabaseError(err) => format!("数据库错误: {}", err),
            AppError::NotFound(msg) => format!("未找到资源: {}", msg),
            AppError::Unauthorized(msg) => format!("认证失败: {}", msg),
            AppError::Forbidden(msg) => format!("权限不足: {}", msg),
            AppError::ValidationError(msg) => format!("验证失败: {}", msg),
            AppError::BusinessError(msg) => format!("业务错误: {}", msg),
            AppError::ConfigError(msg) => format!("配置错误: {}", msg),
            AppError::ExternalApiError(msg) => format!("外部API错误: {}", msg),
            AppError::InternalError(msg) => format!("内部服务器错误: {}", msg),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_response = json!({
            "success": false,
            "error": self.error_message(),
            "status_code": status.as_u16(),
        });

        (status, Json(error_response)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

pub fn not_found(msg: &str) -> AppError {
    AppError::NotFound(msg.to_string())
}

pub fn unauthorized(msg: &str) -> AppError {
    AppError::Unauthorized(msg.to_string())
}

pub fn forbidden(msg: &str) -> AppError {
    AppError::Forbidden(msg.to_string())
}

pub fn validation_error(msg: &str) -> AppError {
    AppError::ValidationError(msg.to_string())
}

pub fn business_error(msg: &str) -> AppError {
    AppError::BusinessError(msg.to_string())
}
