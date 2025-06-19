// Modified: 2025-06-19 16:10:00 EEST
// xaiArtifact: artifact_id="7252ca55-31ea-43bb-8093-b46364acf33f", artifact_version_id="6f7a8b9c-0d1e-2f3a-4b5c-6d7e8f9a0b1c"

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub enum AppError {
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal error: {}", msg),
            )
                .into_response(),
        }
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        AppError::Internal(msg)
    }
}