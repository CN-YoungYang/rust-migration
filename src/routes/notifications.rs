use crate::{
    error::Result,
    models::{AppUser, CreateNotificationRequest, NotificationConfig, UpdateNotificationRequest},
    security::{validate_public_host_resolved, validate_public_http_url_resolved},
    AppState,
};
use axum::http::HeaderName;
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use std::sync::Arc;

fn validate_common(
    notify_type: Option<&str>,
    failure_threshold: Option<i64>,
    balance_threshold: Option<f64>,
    webhook_method: Option<&str>,
    webhook_headers: Option<&str>,
) -> Result<()> {
    if let Some(kind) = notify_type {
        if !["email", "webhook", "telegram"].contains(&kind) {
            return Err(crate::error::AppError::Validation(format!(
                "不支持的通知类型: {}",
                kind
            )));
        }
    }
    if let Some(threshold) = failure_threshold {
        if !(1..=100).contains(&threshold) {
            return Err(crate::error::AppError::Validation(
                "failureThreshold 必须在 1~100 之间".into(),
            ));
        }
    }
    if let Some(threshold) = balance_threshold {
        if threshold < 0.0 {
            return Err(crate::error::AppError::Validation(
                "balanceThreshold 不能小于 0".into(),
            ));
        }
    }
    if let Some(method) = webhook_method {
        if !["POST", "PUT"].contains(&method) {
            return Err(crate::error::AppError::Validation(
                "Webhook 方法仅支持 POST 或 PUT".into(),
            ));
        }
    }
    if let Some(headers_json) = webhook_headers {
        let value: serde_json::Value = serde_json::from_str(headers_json).map_err(|_| {
            crate::error::AppError::Validation("webhookHeaders 必须是 JSON 对象".into())
        })?;
        let object = value.as_object().ok_or_else(|| {
            crate::error::AppError::Validation("webhookHeaders 必须是 JSON 对象".into())
        })?;
        for (name, value) in object {
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                crate::error::AppError::Validation(format!("非法 Webhook Header 名称: {}", name))
            })?;
            if !value.is_string() {
                return Err(crate::error::AppError::Validation(format!(
                    "Webhook Header {} 的值必须是字符串",
                    name
                )));
            }
        }
    }
    Ok(())
}

fn require_non_empty(value: Option<&str>, field: &str) -> Result<()> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(_) => Ok(()),
        None => Err(crate::error::AppError::Validation(format!(
            "{} 不能为空",
            field
        ))),
    }
}

async fn validate_create_request(req: &CreateNotificationRequest) -> Result<()> {
    validate_common(
        Some(req.notify_type.as_str()),
        req.failure_threshold,
        req.balance_threshold,
        req.webhook_method.as_deref(),
        req.webhook_headers.as_deref(),
    )?;

    match req.notify_type.as_str() {
        "email" => {
            require_non_empty(req.email_smtp_host.as_deref(), "SMTP 主机")?;
            let port = req
                .email_smtp_port
                .ok_or_else(|| crate::error::AppError::Validation("SMTP 端口不能为空".into()))?;
            if !(1..=65535).contains(&port) {
                return Err(crate::error::AppError::Validation(
                    "SMTP 端口必须在 1~65535 之间".into(),
                ));
            }
            validate_public_host_resolved(
                req.email_smtp_host.as_deref().unwrap_or_default(),
                port as u16,
                "SMTP 主机",
            )
            .await?;
            require_non_empty(req.email_smtp_user.as_deref(), "SMTP 用户名")?;
            require_non_empty(req.email_smtp_password.as_deref(), "SMTP 密码")?;
            require_non_empty(req.email_from.as_deref(), "发件人")?;
            require_non_empty(req.email_to.as_deref(), "收件人")?;
        }
        "webhook" => {
            let url = req.webhook_url.as_deref().unwrap_or_default();
            require_non_empty(Some(url), "Webhook URL")?;
            validate_public_http_url_resolved(url, "Webhook URL").await?;
        }
        "telegram" => {
            require_non_empty(req.telegram_bot_token.as_deref(), "Telegram Bot Token")?;
            require_non_empty(req.telegram_chat_id.as_deref(), "Telegram Chat ID")?;
        }
        _ => unreachable!("notify_type 已在 validate_common 中校验"),
    }
    Ok(())
}

async fn validate_update_request(
    existing: &NotificationConfig,
    req: &UpdateNotificationRequest,
) -> Result<()> {
    validate_common(
        None,
        req.failure_threshold,
        req.balance_threshold,
        req.webhook_method.as_deref(),
        req.webhook_headers.as_deref(),
    )?;

    let effective_port = req.email_smtp_port.or(existing.email_smtp_port);
    if let Some(host) = req.email_smtp_host.as_deref() {
        require_non_empty(Some(host), "SMTP 主机")?;
        if let Some(port) = effective_port {
            if (1..=65535).contains(&port) {
                validate_public_host_resolved(host, port as u16, "SMTP 主机").await?;
            }
        }
    } else if existing.notify_type == "email" {
        if let Some(host) = existing.email_smtp_host.as_deref() {
            if let Some(port) = effective_port {
                if (1..=65535).contains(&port) {
                    validate_public_host_resolved(host, port as u16, "SMTP 主机").await?;
                }
            }
        }
    }
    if let Some(port) = req.email_smtp_port {
        if !(1..=65535).contains(&port) {
            return Err(crate::error::AppError::Validation(
                "SMTP 端口必须在 1~65535 之间".into(),
            ));
        }
    }
    if let Some(url) = req.webhook_url.as_deref() {
        require_non_empty(Some(url), "Webhook URL")?;
        validate_public_http_url_resolved(url, "Webhook URL").await?;
    } else if existing.notify_type == "webhook" {
        if let Some(url) = existing.webhook_url.as_deref() {
            validate_public_http_url_resolved(url, "Webhook URL").await?;
        }
    }
    Ok(())
}

/// GET /api/notifications - 列出当前用户的通知配置
pub async fn list_notifications(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
) -> Result<Json<serde_json::Value>> {
    let configs = crate::db::list_notifications(&state.db, &user.id).await?;
    Ok(crate::routes::data(configs))
}

/// GET /api/notifications/:id - 获取单个通知配置
pub async fn get_notification(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let config = crate::db::get_notification(&state.db, &id, &user.id).await?;
    Ok(crate::routes::data(config))
}

/// POST /api/notifications - 创建通知配置
pub async fn create_notification(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Json(req): Json<CreateNotificationRequest>,
) -> Result<Json<serde_json::Value>> {
    validate_create_request(&req).await?;
    let config = crate::db::create_notification(&state.db, &user.id, &req).await?;
    Ok(crate::routes::data(config))
}

/// PUT /api/notifications/:id - 更新通知配置
pub async fn update_notification(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNotificationRequest>,
) -> Result<Json<serde_json::Value>> {
    let existing = crate::db::get_notification(&state.db, &id, &user.id).await?;
    validate_update_request(&existing, &req).await?;
    let config = crate::db::update_notification(&state.db, &id, &user.id, &req).await?;
    Ok(crate::routes::data(config))
}

/// DELETE /api/notifications/:id - 删除通知配置
pub async fn delete_notification(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    crate::db::delete_notification(&state.db, &id, &user.id).await?;
    Ok(crate::routes::data(serde_json::json!({ "success": true })))
}

/// POST /api/notifications/:id/test - 测试通知配置
pub async fn test_notification(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let config = crate::db::get_notification(&state.db, &id, &user.id).await?;

    let test_payload = crate::services::notification::NotificationPayload {
        account_name: "测试账户".to_string(),
        site_type: "new-api".to_string(),
        base_url: "https://api.example.com".to_string(),
        status: "failed".to_string(),
        message: "这是一条测试通知".to_string(),
        balance: Some(5.0),
        consecutive_failures: 1,
    };

    match crate::services::notification::send_notification(&config, &test_payload).await {
        Ok(_) => Ok(crate::routes::data(serde_json::json!({
            "success": true,
            "message": "测试通知发送成功"
        }))),
        Err(e) => Ok(crate::routes::data(serde_json::json!({
            "success": false,
            "message": format!("测试通知发送失败: {}", e)
        }))),
    }
}
