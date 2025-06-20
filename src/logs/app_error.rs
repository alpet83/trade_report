use std::fmt;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode; // Изменено
use axum::Json; 
use serde_json::json;

#[derive(Debug)] // Добавлено
pub enum AppError {
    Internal(String),
    NotFound(String),
    BadRequest(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = json!({
            "error": error_message,
        });

        (status, Json(body)).into_response()
    }
}