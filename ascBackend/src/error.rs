use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::fmt;

/// 统一错误类型：所有端点错误返回 { success: false, error: "..." }
#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl AppError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        AppError {
            status,
            message: message.into(),
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, msg)
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, msg)
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, msg)
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, msg)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "success": false, "error": self.message }));
        (self.status, body).into_response()
    }
}

impl From<diesel::result::Error> for AppError {
    fn from(e: diesel::result::Error) -> Self {
        tracing::error!("db error: {e}");
        AppError::internal("数据库操作失败")
    }
}

impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        tracing::error!("pool error: {e}");
        AppError::internal("数据库连接失败")
    }
}

pub type ApiResult<T> = Result<T, AppError>;

/// 统一成功响应：{ success: true, data } 或 { success: true, message, data }
pub fn ok(data: serde_json::Value) -> Json<serde_json::Value> {
    Json(json!({ "success": true, "data": data }))
}

pub fn ok_msg(message: &str, data: serde_json::Value) -> Json<serde_json::Value> {
    Json(json!({ "success": true, "message": message, "data": data }))
}
