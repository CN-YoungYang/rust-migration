use sqlx::SqlitePool;
use crate::models::*;
use crate::error::Result;
use chrono::{Utc, TimeZone, Local};

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

pub async fn create_user(db: &SqlitePool, username: &str, password_hash: &str, role: &str, enabled: bool, note: Option<&str>) -> Result<AppUser> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    
    sqlx::query(
        "INSERT INTO AppUser (id, username, passwordHash, role, enabled, note, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(username)
    .bind(password_hash)
    .bind(role)
    .bind(enabled)
    .bind(note)
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
    site_type: &str,
    base_url: &str,
    user_id: Option<&str>,
    auth_type: &str,
    access_token_enc: Option<&str>,
    cookie_enc: Option<&str>,
    custom_checkin_url: Option<&str>,
    enabled: bool,
    retry_enabled: bool,
    owner_id: &str,
) -> Result<CheckinAccount> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    
    sqlx::query(
        "INSERT INTO CheckinAccount (id, name, siteType, baseUrl, userId, ownerId, authType, accessTokenEnc, cookieEnc, customCheckinUrl, enabled, retryEnabled, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(name)
    .bind(site_type)
    .bind(base_url)
    .bind(user_id)
    .bind(owner_id)
    .bind(auth_type)
    .bind(access_token_enc)
    .bind(cookie_enc)
    .bind(custom_checkin_url)
    .bind(enabled)
    .bind(retry_enabled)
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
    account_id: &str,
    status: &str,
    message: Option<&str>,
    duration_ms: Option<i32>,
    triggered_by: &str,
    raw_response: Option<&str>,
) -> Result<CheckinRun> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    
    sqlx::query(
        "INSERT INTO CheckinRun (id, accountId, status, message, durationMs, triggeredBy, rawResponse, createdAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(account_id)
    .bind(status)
    .bind(message)
    .bind(duration_ms)
    .bind(triggered_by)
    .bind(raw_response)
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
/// 对旧库做幂等迁移：批量签到随机延迟列在 v2.2.2 引入，
/// 已存在的库需要补列。SQLite 不支持 ADD COLUMN IF NOT EXISTS，
/// 故用 try + 忽略“duplicate column”错误。
async fn ensure_setting_columns(db: &SqlitePool) -> Result<()> {
    for col in ["batchDelayMin", "batchDelayMax"] {
        let sql = format!("ALTER TABLE CheckinSetting ADD COLUMN {} INTEGER NOT NULL DEFAULT 0", col);
        if let Err(e) = sqlx::query(&sql).execute(db).await {
            // “duplicate column name” 说明列已存在，可安全忽略
            let msg = e.to_string();
            if !msg.contains("duplicate column") {
                return Err(e.into());
            }
        }
    }
    Ok(())
}

pub async fn get_settings(db: &SqlitePool) -> Result<CheckinSetting> {
    ensure_setting_columns(db).await?;

    let settings = sqlx::query_as::<_, CheckinSetting>(
        "SELECT * FROM CheckinSetting WHERE id = 'global'"
    )
    .fetch_optional(db)
    .await?;
    
    if let Some(s) = settings {
        // 旧库迁移后默认值是 0，这里修正为安全默认（3~10s）
        let mut s = s;
        if s.batch_delay_max <= 0 {
            s.batch_delay_min = 3;
            s.batch_delay_max = 10;
        }
        if s.batch_delay_min < 0 {
            s.batch_delay_min = 0;
        }
        Ok(s)
    } else {
        // Create default settings
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinSetting (id, enabled, windowStart, windowEnd, retryEnabled, maxAttemptsPerDay, batchDelayMin, batchDelayMax, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("global")
        .bind(false)
        .bind("02:00")
        .bind("05:00")
        .bind(true)
        .bind(3)
        .bind(3)   // batchDelayMin 默认 3 秒
        .bind(10)  // batchDelayMax 默认 10 秒
        .bind(now)
        .execute(db)
        .await?;
        
        Box::pin(get_settings(db)).await
    }
}

pub async fn update_settings(
    db: &SqlitePool,
    enabled: Option<bool>,
    window_start: Option<&str>,
    window_end: Option<&str>,
    retry_enabled: Option<bool>,
    max_attempts_per_day: Option<i32>,
    batch_delay_min: Option<i32>,
    batch_delay_max: Option<i32>,
) -> Result<CheckinSetting> {
    let now = Utc::now();
    let current = Box::pin(get_settings(db)).await?;
    
    sqlx::query(
        "UPDATE CheckinSetting SET enabled = ?, windowStart = ?, windowEnd = ?, retryEnabled = ?, maxAttemptsPerDay = ?, batchDelayMin = ?, batchDelayMax = ?, updatedAt = ? WHERE id = 'global'"
    )
    .bind(enabled.unwrap_or(current.enabled))
    .bind(window_start.unwrap_or(&current.window_start))
    .bind(window_end.unwrap_or(&current.window_end))
    .bind(retry_enabled.unwrap_or(current.retry_enabled))
    .bind(max_attempts_per_day.unwrap_or(current.max_attempts_per_day))
    .bind(batch_delay_min.unwrap_or(current.batch_delay_min))
    .bind(batch_delay_max.unwrap_or(current.batch_delay_max))
    .bind(now)
    .execute(db)
    .await?;
    
    Box::pin(get_settings(db)).await
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
    // Cascade: delete runs for accounts owned by this user
    sqlx::query(
        "DELETE FROM CheckinRun WHERE accountId IN (SELECT id FROM CheckinAccount WHERE ownerId = ?)"
    )
    .bind(id)
    .execute(db)
    .await?;

    // Cascade: delete accounts owned by this user
    sqlx::query("DELETE FROM CheckinAccount WHERE ownerId = ?")
        .bind(id)
        .execute(db)
        .await?;

    // Delete the user
    sqlx::query("DELETE FROM AppUser WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn update_account_balance(db: &SqlitePool, id: &str, balance: f64) -> Result<()> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE CheckinAccount SET lastBalance = ?, lastBalanceAt = ?, updatedAt = ? WHERE id = ?"
    )
    .bind(balance)
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}
pub async fn cleanup_checkin_runs(db: &SqlitePool, keep_latest: usize) -> Result<u64> {
    if keep_latest == 0 {
        let result = sqlx::query("DELETE FROM CheckinRun").execute(db).await?;
        return Ok(result.rows_affected());
    }

    let result = sqlx::query(
        "DELETE FROM CheckinRun WHERE id NOT IN (SELECT id FROM CheckinRun ORDER BY createdAt DESC LIMIT ?)"
    )
    .bind(keep_latest as i64)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}
pub async fn list_accounts_by_user(db: &SqlitePool, user_id: &str) -> Result<Vec<CheckinAccount>> {
    let accounts = sqlx::query_as::<_, CheckinAccount>(
        "SELECT * FROM CheckinAccount WHERE ownerId = ? ORDER BY createdAt DESC"
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    Ok(accounts)
}

pub async fn count_runs_by_account_today(db: &SqlitePool, account_id: &str) -> Result<i32> {
    let local_midnight = Local::now().date_naive().and_hms_opt(0, 0, 0).expect("midnight is always valid");
    let today_start_utc = Local.from_local_datetime(&local_midnight).single().expect("invalid midnight").to_utc();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM CheckinRun WHERE accountId = ? AND createdAt >= ?"
    )
    .bind(account_id)
    .bind(today_start_utc)
    .fetch_one(db)
    .await?;
    Ok(count as i32)
}
pub async fn list_runs_by_user(db: &SqlitePool, user_id: &str, limit: usize) -> Result<Vec<CheckinRun>> {
    let runs = sqlx::query_as::<_, CheckinRun>(
        "SELECT r.* FROM CheckinRun r JOIN CheckinAccount a ON r.accountId = a.id WHERE a.ownerId = ? ORDER BY r.createdAt DESC LIMIT ?"
    )
    .bind(user_id)
    .bind(limit as i64)
    .fetch_all(db)
    .await?;
    Ok(runs)
}

pub async fn cleanup_checkin_runs_by_user(db: &SqlitePool, user_id: &str, keep_latest: usize) -> Result<u64> {
    let owned = "SELECT id FROM CheckinAccount WHERE ownerId = ?";
    if keep_latest == 0 {
        let result = sqlx::query(&format!(
            "DELETE FROM CheckinRun WHERE accountId IN ({})", owned
        ))
        .bind(user_id)
        .execute(db)
        .await?;
        return Ok(result.rows_affected());
    }

    let result = sqlx::query(&format!(
        "DELETE FROM CheckinRun WHERE accountId IN ({owned}) AND id NOT IN (SELECT id FROM CheckinRun WHERE accountId IN ({owned}) ORDER BY createdAt DESC LIMIT ?)"
    ))
    .bind(user_id)
    .bind(user_id)
    .bind(keep_latest as i64)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}
