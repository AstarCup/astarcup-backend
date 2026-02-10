use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }

    pub fn success_with_message(data: T, message: &str) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: Some(message.to_string()),
        }
    }

    pub fn empty_success() -> Self {
        Self {
            success: true,
            data: None,
            message: None,
        }
    }

    pub fn success_message(message: &str) -> Self {
        Self {
            success: true,
            data: None,
            message: Some(message.to_string()),
        }
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        let status = if self.success {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        };

        (status, Json(self)).into_response()
    }
}

// Helper functions for common responses
pub fn created<T>(data: T) -> (StatusCode, Json<ApiResponse<T>>)
where
    T: Serialize,
{
    (
        StatusCode::CREATED,
        Json(ApiResponse::success_with_message(data, "Created successfully")),
    )
}

pub fn ok<T>(data: T) -> (StatusCode, Json<ApiResponse<T>>)
where
    T: Serialize,
{
    (StatusCode::OK, Json(ApiResponse::success(data)))
}

pub fn ok_message(message: &str) -> (StatusCode, Json<ApiResponse<()>>) {
    (
        StatusCode::OK,
        Json(ApiResponse::success_message(message)),
    )
}

pub fn no_content() -> (StatusCode, Json<ApiResponse<()>>) {
    (
        StatusCode::NO_CONTENT,
        Json(ApiResponse::empty_success()),
    )
}