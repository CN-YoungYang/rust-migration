use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct NotificationConfig {
    pub id: String,
    pub owner_id: String,
    pub notify_type: String,
    pub enabled: bool,

    // 触发条件
    pub on_failure: bool,
    pub failure_threshold: i64,
    pub on_balance_low: bool,
    pub balance_threshold: Option<f64>,

    // 邮件配置
    pub email_smtp_host: Option<String>,
    pub email_smtp_port: Option<i64>,
    pub email_smtp_user: Option<String>,
    #[serde(skip_serializing)]
    pub email_smtp_password: Option<String>,
    pub email_from: Option<String>,
    pub email_to: Option<String>,

    // Webhook 配置
    pub webhook_url: Option<String>,
    pub webhook_method: Option<String>,
    pub webhook_headers: Option<String>,

    // Telegram 配置
    #[serde(skip_serializing)]
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,

    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNotificationRequest {
    pub notify_type: String,
    pub enabled: Option<bool>,

    // 触发条件
    pub on_failure: Option<bool>,
    pub failure_threshold: Option<i64>,
    pub on_balance_low: Option<bool>,
    pub balance_threshold: Option<f64>,

    // 邮件配置
    pub email_smtp_host: Option<String>,
    pub email_smtp_port: Option<i64>,
    pub email_smtp_user: Option<String>,
    pub email_smtp_password: Option<String>,
    pub email_from: Option<String>,
    pub email_to: Option<String>,

    // Webhook 配置
    pub webhook_url: Option<String>,
    pub webhook_method: Option<String>,
    pub webhook_headers: Option<String>,

    // Telegram 配置
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,

    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotificationRequest {
    pub enabled: Option<bool>,

    // 触发条件
    pub on_failure: Option<bool>,
    pub failure_threshold: Option<i64>,
    pub on_balance_low: Option<bool>,
    pub balance_threshold: Option<f64>,

    // 邮件配置
    pub email_smtp_host: Option<String>,
    pub email_smtp_port: Option<i64>,
    pub email_smtp_user: Option<String>,
    pub email_smtp_password: Option<String>,
    pub email_from: Option<String>,
    pub email_to: Option<String>,

    // Webhook 配置
    pub webhook_url: Option<String>,
    pub webhook_method: Option<String>,
    pub webhook_headers: Option<String>,

    // Telegram 配置
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,

    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FailureCounter {
    pub account_id: String,
    pub consecutive_failures: i64,
    pub last_failed_at: Option<String>,
    pub last_notified_at: Option<String>,
    pub updated_at: String,
}
