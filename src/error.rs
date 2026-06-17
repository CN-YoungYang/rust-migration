use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("认证失败")]
    Unauthorized,

    #[error("资源不存在")]
    NotFound,

    #[error("权限不足")]
    Forbidden,

    #[error("校验失败: {0}")]
    Validation(String),

    #[error("加密错误: {0}")]
    Crypto(String),

    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),

    #[error("内部错误: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, details) = match self {
            AppError::Database(ref e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "数据库错误", None)
            }
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "认证失败", None),
            AppError::NotFound => (StatusCode::NOT_FOUND, "资源不存在", None),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "权限不足", None),
            AppError::Validation(ref msg) => (StatusCode::BAD_REQUEST, "校验失败", Some(msg.clone())),
            AppError::Crypto(ref e) => {
                tracing::error!("Crypto error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "加密错误", None)
            }
            AppError::Http(ref e) => {
                tracing::error!("HTTP request error: {}", e);
                (StatusCode::BAD_GATEWAY, "HTTP 请求失败", None)
            }
            AppError::Internal(ref e) => {
                tracing::error!("Internal error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "内部错误", None)
            }
        };

        let body = if let Some(d) = details {
            Json(json!({ "error": message, "details": d }))
        } else {
            Json(json!({ "error": message }))
        };

        (status, body).into_response()
    }
}



impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Internal(s)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, AppError>;

