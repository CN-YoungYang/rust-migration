use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;

fn deserialize_nullable_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

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
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub balance_threshold: Option<Option<f64>>,

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
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub webhook_headers: Option<Option<String>>,

    // Telegram 配置
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,

    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub note: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FailureCounter {
    pub account_id: String,
    pub consecutive_failures: i64,
    pub last_failed_at: Option<String>,
    pub last_notified_at: Option<String>,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::UpdateNotificationRequest;

    #[test]
    fn update_request_distinguishes_missing_null_and_value_fields() {
        let missing: UpdateNotificationRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.balance_threshold, None);
        assert_eq!(missing.webhook_headers, None);
        assert_eq!(missing.note, None);

        let cleared: UpdateNotificationRequest =
            serde_json::from_str(r#"{"balanceThreshold":null,"webhookHeaders":null,"note":null}"#)
                .unwrap();
        assert_eq!(cleared.balance_threshold, Some(None));
        assert_eq!(cleared.webhook_headers, Some(None));
        assert_eq!(cleared.note, Some(None));

        let updated: UpdateNotificationRequest = serde_json::from_str(
            r#"{"balanceThreshold":2.5,"webhookHeaders":"{\"X-Test\":\"1\"}","note":"ops"}"#,
        )
        .unwrap();
        assert_eq!(updated.balance_threshold, Some(Some(2.5)));
        assert_eq!(
            updated.webhook_headers,
            Some(Some(r#"{"X-Test":"1"}"#.to_string()))
        );
        assert_eq!(updated.note, Some(Some("ops".to_string())));
    }
}
