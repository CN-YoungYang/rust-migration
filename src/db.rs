use sqlx::SqlitePool;
use crate::models::*;
use crate::error::Result;
use chrono::{Utc, TimeZone, Local};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

// Settings 内存缓存：避免每次请求都查 DB（单行表，变更极少）
static SETTINGS_CACHE: std::sync::OnceLock<Arc<RwLock<Option<(CheckinSetting, Instant)>>>> = std::sync::OnceLock::new();
const SETTINGS_CACHE_TTL: Duration = Duration::from_secs(30);

fn settings_cache() -> &'static Arc<RwLock<Option<(CheckinSetting, Instant)>>> {
    SETTINGS_CACHE.get_or_init(|| Arc::new(RwLock::new(None)))
}

// 列表查询排除加密字段和 rawResponse，减少 I/O 和内存开销
const ACCOUNT_LIST_COLUMNS: &str = "\
    id, name, siteType, baseUrl, userId, ownerId, authType, \
    NULL as accessTokenEnc, NULL as cookieEnc, \
    customCheckinUrl, enabled, retryEnabled, note, \
    lastBalance, lastBalanceAt, lastStatus, lastMessage, lastRunAt, \
    createdAt, updatedAt";
const RUN_LIST_COLUMNS: &str = "\
    id, accountId, status, message, durationMs, triggeredBy, \
    NULL as rawResponse, createdAt";

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

/// 轻量查询：只返回 id -> username 映射，避免拉取 passwordHash 等敏感字段
pub async fn list_user_id_name_map(db: &SqlitePool) -> Result<std::collections::HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, username FROM AppUser")
        .fetch_all(db)
        .await?;
    Ok(rows.into_iter().collect())
}

pub async fn create_user(db: &SqlitePool, username: &str, password_hash: &str, role: &str, enabled: bool, note: Option<&str>) -> Result<AppUser> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let user = sqlx::query_as::<_, AppUser>(
        "INSERT INTO AppUser (id, username, passwordHash, role, enabled, note, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(&id)
    .bind(username)
    .bind(password_hash)
    .bind(role)
    .bind(enabled)
    .bind(note)
    .bind(now)
    .bind(now)
    .fetch_one(db)
    .await?;

    Ok(user)
}

// CheckinAccount operations

/// 分页查询账户（供前端列表使用）
pub async fn list_accounts_paginated(db: &SqlitePool, limit: i32, offset: i32) -> Result<Vec<CheckinAccount>> {
    let sql = format!("SELECT {} FROM CheckinAccount ORDER BY createdAt DESC LIMIT ? OFFSET ?", ACCOUNT_LIST_COLUMNS);
    let accounts = sqlx::query_as::<_, CheckinAccount>(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(accounts)
}

/// 分页查询指定用户的账户
pub async fn list_accounts_by_user_paginated(db: &SqlitePool, user_id: &str, limit: i32, offset: i32) -> Result<Vec<CheckinAccount>> {
    let sql = format!("SELECT {} FROM CheckinAccount WHERE ownerId = ? ORDER BY createdAt DESC LIMIT ? OFFSET ?", ACCOUNT_LIST_COLUMNS);
    let accounts = sqlx::query_as::<_, CheckinAccount>(&sql)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(accounts)
}

/// 只查询已启用的账户，供 scheduler 使用（利用 idx_checkin_account_enabled 索引）
pub async fn list_enabled_accounts(db: &SqlitePool) -> Result<Vec<CheckinAccount>> {
    let sql = format!("SELECT {} FROM CheckinAccount WHERE enabled = 1 ORDER BY createdAt DESC", ACCOUNT_LIST_COLUMNS);
    let accounts = sqlx::query_as::<_, CheckinAccount>(&sql)
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

/// 批量查询账户，返回 id -> account 映射（替代逐个 find_account_by_id）
pub async fn find_accounts_by_ids(db: &SqlitePool, ids: &[String]) -> Result<std::collections::HashMap<String, CheckinAccount>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT * FROM CheckinAccount WHERE id IN ({})", placeholders);
    let mut query = sqlx::query_as::<_, CheckinAccount>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    let accounts = query.fetch_all(db).await?;
    Ok(accounts.into_iter().map(|a| (a.id.clone(), a)).collect())
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
    note: Option<&str>,
) -> Result<CheckinAccount> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let account = sqlx::query_as::<_, CheckinAccount>(
        "INSERT INTO CheckinAccount (id, name, siteType, baseUrl, userId, ownerId, authType, accessTokenEnc, cookieEnc, customCheckinUrl, enabled, retryEnabled, note, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
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
    .bind(note)
    .bind(now)
    .bind(now)
    .fetch_one(db)
    .await?;

    Ok(account)
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
pub async fn list_runs(db: &SqlitePool, limit: i32, offset: i32, owner_id: Option<&str>) -> Result<Vec<CheckinRun>> {
    let runs = if let Some(oid) = owner_id {
        let sql = format!(
            "SELECT {} FROM CheckinRun r JOIN CheckinAccount a ON r.accountId = a.id WHERE a.ownerId = ? ORDER BY r.createdAt DESC LIMIT ? OFFSET ?",
            RUN_LIST_COLUMNS.replace("id,", "r.id,").replace("accountId,", "r.accountId,").replace("createdAt", "r.createdAt")
        );
        sqlx::query_as::<_, CheckinRun>(&sql)
            .bind(oid)
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await?
    } else {
        let sql = format!("SELECT {} FROM CheckinRun ORDER BY createdAt DESC LIMIT ? OFFSET ?", RUN_LIST_COLUMNS);
        sqlx::query_as::<_, CheckinRun>(&sql)
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await?
    };
    Ok(runs)
}

pub async fn create_run(
    db: &SqlitePool,
    account_id: &str,
    status: &str,
    message: Option<&str>,
    duration_ms: Option<i64>,
    triggered_by: &str,
    raw_response: Option<&str>,
) -> Result<CheckinRun> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let run = sqlx::query_as::<_, CheckinRun>(
        "INSERT INTO CheckinRun (id, accountId, status, message, durationMs, triggeredBy, rawResponse, createdAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(&id)
    .bind(account_id)
    .bind(status)
    .bind(message)
    .bind(duration_ms)
    .bind(triggered_by)
    .bind(raw_response)
    .bind(now)
    .fetch_one(db)
    .await?;

    Ok(run)
}

// CheckinSetting operations
/// 对旧库做幂等迁移：批量签到随机延迟列在 v2.2.2 引入，
/// 已存在的库需要补列。SQLite 不支持 ADD COLUMN IF NOT EXISTS，
/// 故用 try + 忽略“duplicate column”错误。
pub async fn ensure_setting_columns(db: &SqlitePool) -> Result<()> {
    for col in ["batchDelayMin", "batchDelayMax"] {
        let sql = format!("ALTER TABLE CheckinSetting ADD COLUMN {} INTEGER NOT NULL DEFAULT 0", col);
        if let Err(e) = sqlx::query(&sql).execute(db).await {
            let msg = e.to_string();
            if !msg.contains("duplicate column") {
                return Err(e.into());
            }
        }
    }
    // CheckinAccount.note 列（v2.3.2 引入）
    if let Err(e) = sqlx::query("ALTER TABLE CheckinAccount ADD COLUMN note TEXT").execute(db).await {
        let msg = e.to_string();
        if !msg.contains("duplicate column") {
            return Err(e.into());
        }
    }
    Ok(())
}

pub async fn get_settings(db: &SqlitePool) -> Result<CheckinSetting> {
    // 检查缓存
    {
        let cache = settings_cache().read().unwrap();
        if let Some((settings, cached_at)) = cache.as_ref() {
            if cached_at.elapsed() < SETTINGS_CACHE_TTL {
                return Ok(settings.clone());
            }
        }
    }

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
        // 写入缓存
        {
            let mut cache = settings_cache().write().unwrap();
            *cache = Some((s.clone(), Instant::now()));
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

    let settings = sqlx::query_as::<_, CheckinSetting>(
        "UPDATE CheckinSetting SET enabled = ?, windowStart = ?, windowEnd = ?, retryEnabled = ?, maxAttemptsPerDay = ?, batchDelayMin = ?, batchDelayMax = ?, updatedAt = ? WHERE id = 'global' RETURNING *"
    )
    .bind(enabled.unwrap_or(current.enabled))
    .bind(window_start.unwrap_or(&current.window_start))
    .bind(window_end.unwrap_or(&current.window_end))
    .bind(retry_enabled.unwrap_or(current.retry_enabled))
    .bind(max_attempts_per_day.unwrap_or(current.max_attempts_per_day))
    .bind(batch_delay_min.unwrap_or(current.batch_delay_min))
    .bind(batch_delay_max.unwrap_or(current.batch_delay_max))
    .bind(now)
    .fetch_one(db)
    .await?;

    // 更新缓存
    {
        let mut cache = settings_cache().write().unwrap();
        *cache = Some((settings.clone(), Instant::now()));
    }

    Ok(settings)
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
    note: Option<&str>,
) -> Result<CheckinAccount> {
    let now = Utc::now();
    let current = find_account_by_id(db, id).await?.ok_or(crate::error::AppError::NotFound)?;

    let account = sqlx::query_as::<_, CheckinAccount>(
        "UPDATE CheckinAccount SET name = ?, baseUrl = ?, userId = ?, accessTokenEnc = ?, cookieEnc = ?, customCheckinUrl = ?, enabled = ?, retryEnabled = ?, note = ?, updatedAt = ? WHERE id = ? RETURNING *"
    )
    .bind(name.unwrap_or(&current.name))
    .bind(base_url.unwrap_or(&current.base_url))
    .bind(user_id.or(current.user_id.as_deref()))
    .bind(access_token_enc.or(current.access_token_enc.as_deref()))
    .bind(cookie_enc.or(current.cookie_enc.as_deref()))
    .bind(custom_checkin_url.or(current.custom_checkin_url.as_deref()))
    .bind(enabled.unwrap_or(current.enabled))
    .bind(retry_enabled.unwrap_or(current.retry_enabled))
    .bind(note.or(current.note.as_deref()))
    .bind(now)
    .bind(id)
    .fetch_one(db)
    .await?;

    Ok(account)
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
    let mut tx = db.begin().await?;

    // Cascade: delete runs for accounts owned by this user
    sqlx::query(
        "DELETE FROM CheckinRun WHERE accountId IN (SELECT id FROM CheckinAccount WHERE ownerId = ?)"
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    // Cascade: delete accounts owned by this user
    sqlx::query("DELETE FROM CheckinAccount WHERE ownerId = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Delete the user
    sqlx::query("DELETE FROM AppUser WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
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

/// 批量查询今日各账户签到次数，返回 accountId -> count 映射。
/// 比逐账户 COUNT 更高效（单条 SQL 替代 N 条）。
pub async fn count_runs_today_batch(db: &SqlitePool) -> Result<std::collections::HashMap<String, i32>> {
    let local_midnight = Local::now().date_naive().and_hms_opt(0, 0, 0).expect("midnight is always valid");
    let today_start_utc = Local.from_local_datetime(&local_midnight).single().expect("invalid midnight").to_utc();
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT accountId, COUNT(*) FROM CheckinRun WHERE createdAt >= ? GROUP BY accountId"
    )
    .bind(today_start_utc)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(id, cnt)| (id, cnt as i32)).collect())
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
