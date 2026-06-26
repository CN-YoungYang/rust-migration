use crate::{
    crypto,
    error::Result,
    models::{
        CreateNotificationRequest, FailureCounter, NotificationConfig, UpdateNotificationRequest,
    },
};
use sqlx::SqlitePool;

const NOTIFICATION_COLUMNS: &str = "\
    id, ownerId as owner_id, notifyType as notify_type, enabled,
    onFailure as on_failure, failureThreshold as failure_threshold,
    onBalanceLow as on_balance_low, balanceThreshold as balance_threshold,
    emailSmtpHost as email_smtp_host, emailSmtpPort as email_smtp_port,
    emailSmtpUser as email_smtp_user, emailSmtpPassword as email_smtp_password,
    emailFrom as email_from, emailTo as email_to,
    webhookUrl as webhook_url, webhookMethod as webhook_method, webhookHeaders as webhook_headers,
    telegramBotToken as telegram_bot_token, telegramChatId as telegram_chat_id,
    note, createdAt as created_at, updatedAt as updated_at";

const FAILURE_COUNTER_COLUMNS: &str = "\
    accountId as account_id, consecutiveFailures as consecutive_failures,
    lastFailedAt as last_failed_at, lastNotifiedAt as last_notified_at,
    updatedAt as updated_at";

/// 列出用户的所有通知配置
pub async fn list_notifications(
    pool: &SqlitePool,
    owner_id: &str,
) -> Result<Vec<NotificationConfig>> {
    let sql = format!(
        "SELECT {} FROM NotificationConfig WHERE ownerId = ? ORDER BY createdAt DESC",
        NOTIFICATION_COLUMNS
    );
    let configs = sqlx::query_as::<_, NotificationConfig>(&sql)
        .bind(owner_id)
        .fetch_all(pool)
        .await?;

    Ok(configs)
}

/// 获取单个通知配置
pub async fn get_notification(
    pool: &SqlitePool,
    id: &str,
    owner_id: &str,
) -> Result<NotificationConfig> {
    let sql = format!(
        "SELECT {} FROM NotificationConfig WHERE id = ? AND ownerId = ?",
        NOTIFICATION_COLUMNS
    );
    let config = sqlx::query_as::<_, NotificationConfig>(&sql)
        .bind(id)
        .bind(owner_id)
        .fetch_optional(pool)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;

    Ok(config)
}

/// 创建通知配置
pub async fn create_notification(
    pool: &SqlitePool,
    owner_id: &str,
    req: &CreateNotificationRequest,
) -> Result<NotificationConfig> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // 加密敏感字段
    let email_smtp_password = req
        .email_smtp_password
        .as_ref()
        .map(|pwd| crypto::encrypt(pwd))
        .transpose()?;

    let telegram_bot_token = req
        .telegram_bot_token
        .as_ref()
        .map(|token| crypto::encrypt(token))
        .transpose()?;

    sqlx::query(
        "INSERT INTO NotificationConfig (
            id, ownerId, notifyType, enabled,
            onFailure, failureThreshold, onBalanceLow, balanceThreshold,
            emailSmtpHost, emailSmtpPort, emailSmtpUser, emailSmtpPassword, emailFrom, emailTo,
            webhookUrl, webhookMethod, webhookHeaders,
            telegramBotToken, telegramChatId,
            note, createdAt, updatedAt
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(owner_id)
    .bind(&req.notify_type)
    .bind(req.enabled.unwrap_or(true))
    .bind(req.on_failure.unwrap_or(true))
    .bind(req.failure_threshold.unwrap_or(1))
    .bind(req.on_balance_low.unwrap_or(false))
    .bind(req.balance_threshold)
    .bind(&req.email_smtp_host)
    .bind(req.email_smtp_port)
    .bind(&req.email_smtp_user)
    .bind(&email_smtp_password)
    .bind(&req.email_from)
    .bind(&req.email_to)
    .bind(&req.webhook_url)
    .bind(req.webhook_method.as_ref().unwrap_or(&"POST".to_string()))
    .bind(&req.webhook_headers)
    .bind(&telegram_bot_token)
    .bind(&req.telegram_chat_id)
    .bind(&req.note)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    get_notification(pool, &id, owner_id).await
}

/// 更新通知配置
pub async fn update_notification(
    pool: &SqlitePool,
    id: &str,
    owner_id: &str,
    req: &UpdateNotificationRequest,
) -> Result<NotificationConfig> {
    // 先验证是否存在
    get_notification(pool, id, owner_id).await?;

    let now = chrono::Utc::now().to_rfc3339();

    // 加密敏感字段
    let email_smtp_password = req
        .email_smtp_password
        .as_ref()
        .map(|pwd| crypto::encrypt(pwd))
        .transpose()?;

    let telegram_bot_token = req
        .telegram_bot_token
        .as_ref()
        .map(|token| crypto::encrypt(token))
        .transpose()?;

    // 构建动态更新 SQL
    let mut updates = Vec::new();
    let mut query = "UPDATE NotificationConfig SET ".to_string();

    if req.enabled.is_some() {
        updates.push("enabled = ?");
    }
    if req.on_failure.is_some() {
        updates.push("onFailure = ?");
    }
    if req.failure_threshold.is_some() {
        updates.push("failureThreshold = ?");
    }
    if req.on_balance_low.is_some() {
        updates.push("onBalanceLow = ?");
    }
    if req.balance_threshold.is_some() {
        updates.push("balanceThreshold = ?");
    }
    if req.email_smtp_host.is_some() {
        updates.push("emailSmtpHost = ?");
    }
    if req.email_smtp_port.is_some() {
        updates.push("emailSmtpPort = ?");
    }
    if req.email_smtp_user.is_some() {
        updates.push("emailSmtpUser = ?");
    }
    if email_smtp_password.is_some() {
        updates.push("emailSmtpPassword = ?");
    }
    if req.email_from.is_some() {
        updates.push("emailFrom = ?");
    }
    if req.email_to.is_some() {
        updates.push("emailTo = ?");
    }
    if req.webhook_url.is_some() {
        updates.push("webhookUrl = ?");
    }
    if req.webhook_method.is_some() {
        updates.push("webhookMethod = ?");
    }
    if req.webhook_headers.is_some() {
        updates.push("webhookHeaders = ?");
    }
    if telegram_bot_token.is_some() {
        updates.push("telegramBotToken = ?");
    }
    if req.telegram_chat_id.is_some() {
        updates.push("telegramChatId = ?");
    }
    if req.note.is_some() {
        updates.push("note = ?");
    }

    updates.push("updatedAt = ?");

    query.push_str(&updates.join(", "));
    query.push_str(" WHERE id = ? AND ownerId = ?");

    let mut q = sqlx::query(&query);

    if let Some(v) = req.enabled {
        q = q.bind(v);
    }
    if let Some(v) = req.on_failure {
        q = q.bind(v);
    }
    if let Some(v) = req.failure_threshold {
        q = q.bind(v);
    }
    if let Some(v) = req.on_balance_low {
        q = q.bind(v);
    }
    if let Some(v) = req.balance_threshold {
        q = q.bind(v);
    }
    if let Some(v) = &req.email_smtp_host {
        q = q.bind(v);
    }
    if let Some(v) = req.email_smtp_port {
        q = q.bind(v);
    }
    if let Some(v) = &req.email_smtp_user {
        q = q.bind(v);
    }
    if let Some(v) = &email_smtp_password {
        q = q.bind(v);
    }
    if let Some(v) = &req.email_from {
        q = q.bind(v);
    }
    if let Some(v) = &req.email_to {
        q = q.bind(v);
    }
    if let Some(v) = &req.webhook_url {
        q = q.bind(v);
    }
    if let Some(v) = &req.webhook_method {
        q = q.bind(v);
    }
    if let Some(v) = &req.webhook_headers {
        q = q.bind(v);
    }
    if let Some(v) = &telegram_bot_token {
        q = q.bind(v);
    }
    if let Some(v) = &req.telegram_chat_id {
        q = q.bind(v);
    }
    if let Some(v) = &req.note {
        q = q.bind(v);
    }

    q = q.bind(&now);
    q = q.bind(id);
    q = q.bind(owner_id);

    q.execute(pool).await?;

    get_notification(pool, id, owner_id).await
}

/// 删除通知配置
pub async fn delete_notification(pool: &SqlitePool, id: &str, owner_id: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM NotificationConfig WHERE id = ? AND ownerId = ?")
        .bind(id)
        .bind(owner_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(crate::error::AppError::NotFound);
    }

    Ok(())
}

/// 获取或创建失败计数器
pub async fn get_or_create_failure_counter(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<FailureCounter> {
    let sql = format!(
        "SELECT {} FROM FailureCounter WHERE accountId = ?",
        FAILURE_COUNTER_COLUMNS
    );
    let counter = sqlx::query_as::<_, FailureCounter>(&sql)
        .bind(account_id)
        .fetch_optional(pool)
        .await?;

    if let Some(c) = counter {
        return Ok(c);
    }

    // 创建新的计数器
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO FailureCounter (accountId, consecutiveFailures, updatedAt) VALUES (?, 0, ?)",
    )
    .bind(account_id)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(FailureCounter {
        account_id: account_id.to_string(),
        consecutive_failures: 0,
        last_failed_at: None,
        last_notified_at: None,
        updated_at: now,
    })
}

/// 增加失败计数
pub async fn increment_failure_counter(pool: &SqlitePool, account_id: &str) -> Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO FailureCounter (accountId, consecutiveFailures, lastFailedAt, updatedAt)
         VALUES (?, 1, ?, ?)
         ON CONFLICT(accountId) DO UPDATE SET
             consecutiveFailures = consecutiveFailures + 1,
             lastFailedAt = excluded.lastFailedAt,
             updatedAt = excluded.updatedAt",
    )
    .bind(account_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    let counter = get_or_create_failure_counter(pool, account_id).await?;
    Ok(counter.consecutive_failures)
}

/// 重置失败计数
pub async fn reset_failure_counter(pool: &SqlitePool, account_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE FailureCounter SET consecutiveFailures = 0, updatedAt = ? WHERE accountId = ?",
    )
    .bind(&now)
    .bind(account_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 更新最后通知时间
pub async fn update_last_notified(pool: &SqlitePool, account_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query("UPDATE FailureCounter SET lastNotifiedAt = ?, updatedAt = ? WHERE accountId = ?")
        .bind(&now)
        .bind(&now)
        .bind(account_id)
        .execute(pool)
        .await?;

    Ok(())
}
