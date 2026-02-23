use axum::{
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse,Response},
    Json,
};
use sea_orm::{DbErr, Iden};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Bad request:{0}")]
    BadRequest(String),

    #[error("Validation error:{0}")]
    Validation(String),

    #[error("Json parsing error: {0}")]
    JsonRejection(#[from] JsonRejection),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            Error::Database(DbErr::RecordNotFound(_)) => (StatusCode::NOT_FOUND, "Database RecordNotFound".to_string()),
            Error::Database(DbErr::Query(_)) => (StatusCode::BAD_REQUEST, "Query Failed".to_string()),
            Error::Database(e) => { tracing::error!("Database error {:?}",e);(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string()) },
            Error::Io(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()),
            Error::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.to_string()),
            Error::Validation(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()),
            Error::JsonRejection(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()),
        };
        let body = Json(json!({
            "error": {
                "code": status.as_u16(),
                "message": error_message,
            }
        }));
        (status,body).into_response()
    }
}
pub type Result<T> = std::result::Result<T, Error>;