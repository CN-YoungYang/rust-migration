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

    #[error("资源冲突: {0}")]
    Conflict(String),

    #[error("加密错误: {0}")]
    Crypto(String),

    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("内部错误: {0}")]
    Internal(String),
}

impl AppError {
    /// 面向用户的脱敏消息，供非 HTTP 错误响应场景使用（如 CSV 导入的逐行错误列表）。
    /// 规则与 `IntoResponse` 保持一致：不泄露 sqlx/SMTP/内部实现细节（Low-8）。
    pub fn user_message(&self) -> String {
        match self {
            AppError::Database(e) => database_user_message(e).to_string(),
            AppError::Unauthorized => "认证失败，请重新登录".into(),
            AppError::NotFound => "资源不存在".into(),
            AppError::Forbidden => "权限不足".into(),
            AppError::Validation(msg) => msg.clone(),
            AppError::Conflict(msg) => msg.clone(),
            AppError::Crypto(_) => "加密操作失败，请检查加密密钥配置是否正确".into(),
            AppError::Http(e) => http_error_message(e).to_string(),
            AppError::Io(_) => "外部服务连接失败，请检查网络连接或服务配置".into(),
            AppError::Internal(_) => "服务内部错误".into(),
        }
    }
}

fn database_user_message(e: &sqlx::Error) -> &'static str {
    let raw = e.to_string();
    if raw.contains("UNIQUE constraint failed") {
        "该记录已存在，请检查是否重复"
    } else if raw.contains("FOREIGN KEY constraint failed") {
        "操作失败：存在关联数据"
    } else {
        "服务暂时不可用，请稍后重试"
    }
}

fn http_error_message(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "请求超时，请检查网络连接或稍后重试"
    } else if e.is_connect() {
        "网络连接失败，请检查站点 URL 是否正确"
    } else if e.status().map(|s| s.as_u16()).unwrap_or(0) == 401 {
        "站点认证失败，请检查 Token 或 Cookie 是否正确"
    } else if e.status().map(|s| s.as_u16()).unwrap_or(0) == 404 {
        "站点接口不存在，请检查 URL 配置"
    } else {
        "站点请求失败，请稍后重试"
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, details) = match self {
            AppError::Database(ref e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    database_user_message(e),
                    None,
                )
            }
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "认证失败，请重新登录",
                Some("会话已过期或无效".to_string()),
            ),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "资源不存在",
                Some("请求的资源未找到或已被删除".to_string()),
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "权限不足",
                Some("您没有权限执行此操作".to_string()),
            ),
            AppError::Validation(ref msg) => {
                (StatusCode::BAD_REQUEST, "输入验证失败", Some(msg.clone()))
            }
            AppError::Conflict(ref msg) => (StatusCode::CONFLICT, "资源冲突", Some(msg.clone())),
            AppError::Crypto(ref e) => {
                tracing::error!("Crypto error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "加密操作失败",
                    Some("请检查加密密钥配置是否正确".to_string()),
                )
            }
            AppError::Http(ref e) => {
                tracing::error!("HTTP request error: {}", e);
                (StatusCode::BAD_GATEWAY, http_error_message(e), None)
            }
            AppError::Io(ref e) => {
                tracing::error!("IO error: {}", e);
                (
                    StatusCode::BAD_GATEWAY,
                    "外部服务连接失败",
                    Some("请检查网络连接或服务配置".to_string()),
                )
            }
            AppError::Internal(ref e) => {
                tracing::error!("Internal error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "服务内部错误",
                    Some("请联系管理员或稍后重试".to_string()),
                )
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

impl From<csv::Error> for AppError {
    fn from(e: csv::Error) -> Self {
        AppError::Internal(format!("CSV 错误: {}", e))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
