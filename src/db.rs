use sqlx::SqlitePool;
use crate::models::*;
use crate::error::Result;
use chrono::Utc;

pub async fn get_pool(databaseUrl: &str) -> Result<SqlitePool> {
    let pool = SqlitePool::connect(databaseUrl).await?;
    Ok(pool)
}

// AppUser operations
pub async fn find_user_by_username(db: &SqlitePool, username: &str) -> Result<Option<AppUser>> {
    let user = sqlx::query_as::<_, AppUser>(
        "SELECT * FROM AppUser WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(db)
    .await?;
    Ok(user)
}

pub async fn find_user_by_id(db: &SqlitePool, id: &str) -> Result<Option<AppUser>> {
    let user = sqlx::query_as::<_, AppUser>(
        "SELECT * FROM AppUser WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(user)
}

pub async fn list_users(db: &SqlitePool) -> Result<Vec<AppUser>> {
    let users = sqlx::query_as::<_, AppUser>(
        "SELECT * FROM AppUser ORDER BY createdAt DESC"
    )
    .fetch_all(db)
    .await?;
    Ok(users)
}

pub async fn create_user(db: &SqlitePool, username: &str, passwordHash: &str, role: &str) -> Result<AppUser> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    
    sqlx::query(
        "INSERT INTO AppUser (id, username, passwordHash, role, enabled, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(username)
    .bind(passwordHash)
    .bind(role)
    .bind(true)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?;
    
    find_user_by_id(db, &id).await?.ok_or(crate::error::AppError::NotFound)
}

// CheckinAccount operations
pub async fn list_accounts(db: &SqlitePool) -> Result<Vec<CheckinAccount>> {
    let accounts = sqlx::query_as::<_, CheckinAccount>(
        "SELECT * FROM CheckinAccount ORDER BY createdAt DESC"
    )
    .fetch_all(db)
    .await?;
    Ok(accounts)
}

pub async fn find_account_by_id(db: &SqlitePool, id: &str) -> Result<Option<CheckinAccount>> {
    let account = sqlx::query_as::<_, CheckinAccount>(
        "SELECT * FROM CheckinAccount WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(account)
}

pub async fn create_account(
    db: &SqlitePool,
    name: &str,
    siteType: &str,
    baseUrl: &str,
    userId: Option<&str>,
    authType: &str,
    accessTokenEnc: Option<&str>,
    cookieEnc: Option<&str>,
    customCheckinUrl: Option<&str>,
    enabled: bool,
    retryEnabled: bool,
) -> Result<CheckinAccount> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    
    sqlx::query(
        "INSERT INTO CheckinAccount (id, name, siteType, baseUrl, userId, authType, accessTokenEnc, cookieEnc, customCheckinUrl, enabled, retryEnabled, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(name)
    .bind(siteType)
    .bind(baseUrl)
    .bind(userId)
    .bind(authType)
    .bind(accessTokenEnc)
    .bind(cookieEnc)
    .bind(customCheckinUrl)
    .bind(enabled)
    .bind(retryEnabled)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?;
    
    find_account_by_id(db, &id).await?.ok_or(crate::error::AppError::NotFound)
}

pub async fn update_account_status(
    db: &SqlitePool,
    id: &str,
    status: &str,
    message: Option<&str>,
) -> Result<()> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE CheckinAccount SET lastStatus = ?, lastMessage = ?, lastRunAt = ?, updatedAt = ? WHERE id = ?"
    )
    .bind(status)
    .bind(message)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_account(db: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM CheckinAccount WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

// CheckinRun operations
pub async fn list_runs(db: &SqlitePool, limit: i32) -> Result<Vec<CheckinRun>> {
    let runs = sqlx::query_as::<_, CheckinRun>(
        "SELECT * FROM CheckinRun ORDER BY createdAt DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(runs)
}

pub async fn create_run(
    db: &SqlitePool,
    accountId: &str,
    status: &str,
    message: Option<&str>,
    durationMs: Option<i32>,
    triggeredBy: &str,
    rawResponse: Option<&str>,
) -> Result<CheckinRun> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    
    sqlx::query(
        "INSERT INTO CheckinRun (id, accountId, status, message, durationMs, triggeredBy, rawResponse, createdAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(accountId)
    .bind(status)
    .bind(message)
    .bind(durationMs)
    .bind(triggeredBy)
    .bind(rawResponse)
    .bind(now)
    .execute(db)
    .await?;
    
    let run = sqlx::query_as::<_, CheckinRun>(
        "SELECT * FROM CheckinRun WHERE id = ?"
    )
    .bind(&id)
    .fetch_one(db)
    .await?;
    
    Ok(run)
}

// CheckinSetting operations
pub async fn get_settings(db: &SqlitePool) -> Result<CheckinSetting> {
    let settings = sqlx::query_as::<_, CheckinSetting>(
        "SELECT * FROM CheckinSetting WHERE id = 'global'"
    )
    .fetch_optional(db)
    .await?;
    
    if let Some(s) = settings {
        Ok(s)
    } else {
        // Create default settings
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinSetting (id, enabled, windowStart, windowEnd, retryEnabled, maxAttemptsPerDay, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("global")
        .bind(false)
        .bind("02:00")
        .bind("05:00")
        .bind(true)
        .bind(3)
        .bind(now)
        .execute(db)
        .await?;
        
        get_settings(db).await
    }
}

pub async fn update_settings(
    db: &SqlitePool,
    enabled: Option<bool>,
    windowStart: Option<&str>,
    windowEnd: Option<&str>,
    retryEnabled: Option<bool>,
    maxAttemptsPerDay: Option<i32>,
) -> Result<CheckinSetting> {
    let now = Utc::now();
    let current = get_settings(db).await?;
    
    sqlx::query(
        "UPDATE CheckinSetting SET enabled = ?, windowStart = ?, windowEnd = ?, retryEnabled = ?, maxAttemptsPerDay = ?, updatedAt = ? WHERE id = 'global'"
    )
    .bind(enabled.unwrap_or(current.enabled))
    .bind(windowStart.unwrap_or(&current.windowStart))
    .bind(windowEnd.unwrap_or(&current.windowEnd))
    .bind(retryEnabled.unwrap_or(current.retryEnabled))
    .bind(maxAttemptsPerDay.unwrap_or(current.maxAttemptsPerDay))
    .bind(now)
    .execute(db)
    .await?;
    
    get_settings(db).await
}

pub async fn update_account(
    db: &SqlitePool,
    id: &str,
    name: Option<&str>,
    base_url: Option<&str>,
    user_id: Option<&str>,
    access_token_enc: Option<&str>,
    cookie_enc: Option<&str>,
    custom_checkin_url: Option<&str>,
    enabled: Option<bool>,
    retry_enabled: Option<bool>,
) -> Result<()> {
    let now = Utc::now();
    let current = find_account_by_id(db, id).await?.ok_or(crate::error::AppError::NotFound)?;
    
    sqlx::query(
        "UPDATE CheckinAccount SET name = ?, baseUrl = ?, userId = ?, accessTokenEnc = ?, cookieEnc = ?, customCheckinUrl = ?, enabled = ?, retryEnabled = ?, updatedAt = ? WHERE id = ?"
    )
    .bind(name.unwrap_or(&current.name))
    .bind(base_url.unwrap_or(&current.base_url))
    .bind(user_id.or(current.user_id.as_deref()))
    .bind(access_token_enc.or(current.access_token_enc.as_deref()))
    .bind(cookie_enc.or(current.cookie_enc.as_deref()))
    .bind(custom_checkin_url.or(current.custom_checkin_url.as_deref()))
    .bind(enabled.unwrap_or(current.enabled))
    .bind(retry_enabled.unwrap_or(current.retry_enabled))
    .bind(now)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}
pub async fn update_user(
    db: &SqlitePool,
    id: &str,
    username: Option<&str>,
    password_hash: Option<&str>,
    role: Option<&str>,
    enabled: Option<bool>,
    note: Option<&str>,
) -> Result<()> {
    let now = Utc::now();
    let current = find_user_by_id(db, id).await?.ok_or(crate::error::AppError::NotFound)?;
    
    sqlx::query(
        "UPDATE AppUser SET username = ?, passwordHash = ?, role = ?, enabled = ?, note = ?, updatedAt = ? WHERE id = ?"
    )
    .bind(username.unwrap_or(&current.username))
    .bind(password_hash.unwrap_or(&current.password_hash))
    .bind(role.unwrap_or(&current.role))
    .bind(enabled.unwrap_or(current.enabled))
    .bind(note.or(current.note.as_deref()))
    .bind(now)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_user(db: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM AppUser WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}
