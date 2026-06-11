use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Authentication failed")]
    Unauthorized,
    
    #[error("Not found")]
    NotFound,
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Crypto error: {0}")]
    Crypto(String),
    
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found"),
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, "Validation error"),
            AppError::Crypto(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Crypto error"),
            AppError::Http(_) => (StatusCode::BAD_GATEWAY, "HTTP request failed"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
        };

        let body = Json(json!({
            "error": message,
            "details": self.to_string(),
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
