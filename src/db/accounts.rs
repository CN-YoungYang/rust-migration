use super::types::{AccountFilter, CreateAccountRequest, UpdateAccountRequest};
use crate::error::Result;
use crate::models::CheckinAccount;
use chrono::Utc;
use sqlx::SqlitePool;

/// Column list for account queries (excludes encrypted fields to reduce I/O)
const ACCOUNT_LIST_COLUMNS: &str = "\
    id, name, siteType, baseUrl, userId, ownerId, authType, \
    NULL as accessTokenEnc, NULL as cookieEnc, \
    customCheckinUrl, enabled, retryEnabled, note, \
    lastBalance, lastBalanceAt, lastStatus, lastMessage, lastRunAt, \
    createdAt, updatedAt";

/// List accounts with filters and pagination
pub async fn list_accounts_filtered(
    db: &SqlitePool,
    filter: &AccountFilter,
) -> Result<Vec<CheckinAccount>> {
    let mut sql = format!(
        "SELECT {} FROM CheckinAccount WHERE 1=1",
        ACCOUNT_LIST_COLUMNS
    );

    if filter.owner_id.is_some() {
        sql.push_str(" AND ownerId = ?");
    }
    if filter.site_type.is_some() {
        sql.push_str(" AND siteType = ?");
    }
    if filter.enabled.is_some() {
        sql.push_str(" AND enabled = ?");
    }
    if let Some(ref status) = filter.last_status {
        if status == "never" {
            sql.push_str(" AND lastStatus IS NULL");
        } else if status == "not_today" {
            // 今日未签到：lastRunAt 为 NULL 或不在今天（本地时区）
            sql.push_str(" AND (lastRunAt IS NULL OR DATE(lastRunAt, 'localtime') < DATE('now', 'localtime'))");
        } else {
            sql.push_str(" AND lastStatus = ?");
        }
    }
    if filter.keyword.is_some() {
        sql.push_str(" AND (name LIKE ? OR baseUrl LIKE ? OR note LIKE ?)");
    }

    sql.push_str(" ORDER BY createdAt DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query_as::<_, CheckinAccount>(&sql);

    if let Some(ref oid) = filter.owner_id {
        query = query.bind(oid);
    }
    if let Some(ref st) = filter.site_type {
        query = query.bind(st);
    }
    if let Some(e) = filter.enabled {
        query = query.bind(e);
    }
    if let Some(ref status) = filter.last_status {
        if status != "never" && status != "not_today" {
            query = query.bind(status);
        }
    }
    if let Some(ref kw) = filter.keyword {
        let pattern = format!("%{}%", kw);
        query = query
            .bind(pattern.clone())
            .bind(pattern.clone())
            .bind(pattern);
    }

    query = query.bind(filter.limit).bind(filter.offset);

    let accounts = query.fetch_all(db).await?;
    Ok(accounts)
}

/// List only enabled accounts owned by enabled users.
pub async fn list_enabled_accounts(db: &SqlitePool) -> Result<Vec<CheckinAccount>> {
    let sql = format!(
        "SELECT {} FROM CheckinAccount \
         WHERE enabled = 1 AND ownerId IN (SELECT id FROM AppUser WHERE enabled = 1) \
         ORDER BY createdAt DESC",
        ACCOUNT_LIST_COLUMNS
    );
    let accounts = sqlx::query_as::<_, CheckinAccount>(&sql)
        .fetch_all(db)
        .await?;
    Ok(accounts)
}

/// Find account by ID (includes encrypted fields for check-in operations)
pub async fn find_account_by_id(db: &SqlitePool, id: &str) -> Result<Option<CheckinAccount>> {
    let account = sqlx::query_as::<_, CheckinAccount>(
        "SELECT id, name, siteType, baseUrl, userId, ownerId, authType, \
         accessTokenEnc, cookieEnc, customCheckinUrl, enabled, retryEnabled, note, \
         lastBalance, lastBalanceAt, lastStatus, lastMessage, lastRunAt, \
         createdAt, updatedAt \
         FROM CheckinAccount WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(account)
}

/// Batch query accounts, returns id -> account mapping (replaces N+1 find_account_by_id)
pub async fn find_accounts_by_ids(
    db: &SqlitePool,
    ids: &[String],
) -> Result<std::collections::HashMap<String, CheckinAccount>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, name, siteType, baseUrl, userId, ownerId, authType, \
         accessTokenEnc, cookieEnc, customCheckinUrl, enabled, retryEnabled, note, \
         lastBalance, lastBalanceAt, lastStatus, lastMessage, lastRunAt, \
         createdAt, updatedAt \
         FROM CheckinAccount WHERE id IN ({})",
        placeholders
    );
    let mut query = sqlx::query_as::<_, CheckinAccount>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    let accounts = query.fetch_all(db).await?;
    Ok(accounts.into_iter().map(|a| (a.id.clone(), a)).collect())
}

/// Create a new account
pub async fn create_account(db: &SqlitePool, req: &CreateAccountRequest) -> Result<CheckinAccount> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let account = sqlx::query_as::<_, CheckinAccount>(
        "INSERT INTO CheckinAccount (id, name, siteType, baseUrl, userId, ownerId, authType, accessTokenEnc, cookieEnc, customCheckinUrl, enabled, retryEnabled, note, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.site_type)
    .bind(&req.base_url)
    .bind(req.user_id.as_deref())
    .bind(&req.owner_id)
    .bind(&req.auth_type)
    .bind(req.access_token_enc.as_deref())
    .bind(req.cookie_enc.as_deref())
    .bind(req.custom_checkin_url.as_deref())
    .bind(req.enabled)
    .bind(req.retry_enabled)
    .bind(req.note.as_deref())
    .bind(now)
    .bind(now)
    .fetch_one(db)
    .await?;

    Ok(account)
}

/// Update account details
pub async fn update_account(
    db: &SqlitePool,
    id: &str,
    req: &UpdateAccountRequest,
) -> Result<CheckinAccount> {
    let now = Utc::now();
    let current = find_account_by_id(db, id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;

    // Three-state handling: None=keep original, Some(None)=clear to NULL, Some(Some(v))=set new value
    let resolve = |cur: &Option<String>, new: Option<Option<String>>| -> Option<Option<String>> {
        match new {
            None => cur.as_ref().map(|s| Some(s.clone())), // Keep original
            Some(None) => Some(None),                      // Clear to NULL
            Some(Some(v)) => Some(Some(v)),                // Set new value
        }
    };

    let new_user_id = resolve(&current.user_id, req.user_id.clone());
    let new_access_token_enc = resolve(&current.access_token_enc, req.access_token_enc.clone());
    let new_cookie_enc = resolve(&current.cookie_enc, req.cookie_enc.clone());
    let new_custom_checkin_url =
        resolve(&current.custom_checkin_url, req.custom_checkin_url.clone());
    let new_note = resolve(&current.note, req.note.clone());

    let account = sqlx::query_as::<_, CheckinAccount>(
        "UPDATE CheckinAccount SET name = ?, baseUrl = ?, userId = ?, accessTokenEnc = ?, cookieEnc = ?, customCheckinUrl = ?, enabled = ?, retryEnabled = ?, note = ?, updatedAt = ? WHERE id = ? RETURNING *"
    )
    .bind(req.name.as_ref().unwrap_or(&current.name))
    .bind(req.base_url.as_ref().unwrap_or(&current.base_url))
    .bind(new_user_id.flatten().as_deref())
    .bind(new_access_token_enc.flatten().as_deref())
    .bind(new_cookie_enc.flatten().as_deref())
    .bind(new_custom_checkin_url.flatten().as_deref())
    .bind(req.enabled.unwrap_or(current.enabled))
    .bind(req.retry_enabled.unwrap_or(current.retry_enabled))
    .bind(new_note.flatten().as_deref())
    .bind(now)
    .bind(id)
    .fetch_one(db)
    .await?;

    Ok(account)
}

/// Update account balance
pub async fn update_account_balance(db: &SqlitePool, id: &str, balance: f64) -> Result<()> {
    let now = Utc::now();
    sqlx::query("UPDATE CheckinAccount SET lastBalance = ?, lastBalanceAt = ? WHERE id = ?")
        .bind(balance)
        .bind(now)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Delete account (runs are deleted by CASCADE)
pub async fn delete_account(db: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM CheckinAccount WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");

        sqlx::query(
            "CREATE TABLE AppUser (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                passwordHash TEXT NOT NULL,
                role TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                note TEXT,
                createdAt TEXT NOT NULL,
                updatedAt TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("user table should be created");

        sqlx::query(
            "CREATE TABLE CheckinAccount (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                siteType TEXT NOT NULL,
                baseUrl TEXT NOT NULL,
                userId TEXT,
                ownerId TEXT,
                authType TEXT NOT NULL,
                accessTokenEnc TEXT,
                cookieEnc TEXT,
                customCheckinUrl TEXT,
                enabled INTEGER NOT NULL,
                retryEnabled INTEGER NOT NULL,
                lastBalance REAL,
                lastBalanceAt TEXT,
                lastStatus TEXT,
                lastMessage TEXT,
                lastRunAt TEXT,
                note TEXT,
                createdAt TEXT NOT NULL,
                updatedAt TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("account table should be created");

        pool
    }

    async fn insert_user(pool: &SqlitePool, id: &str, enabled: bool) {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO AppUser (id, username, passwordHash, role, enabled, createdAt, updatedAt)
             VALUES (?, ?, 'hash', 'USER', ?, ?, ?)",
        )
        .bind(id)
        .bind(format!("user-{id}"))
        .bind(enabled)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("user should be inserted");
    }

    async fn insert_account(pool: &SqlitePool, id: &str, owner_id: &str, enabled: bool) {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinAccount (
                id, name, siteType, baseUrl, ownerId, authType,
                enabled, retryEnabled, createdAt, updatedAt
             ) VALUES (?, ?, 'new-api', 'https://example.com', ?, 'access_token', ?, 1, ?, ?)",
        )
        .bind(id)
        .bind(format!("account-{id}"))
        .bind(owner_id)
        .bind(enabled)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("account should be inserted");
    }

    #[tokio::test]
    async fn list_enabled_accounts_skips_disabled_users() {
        let pool = test_pool().await;
        insert_user(&pool, "active-user", true).await;
        insert_user(&pool, "disabled-user", false).await;
        insert_account(&pool, "active-account", "active-user", true).await;
        insert_account(&pool, "disabled-owner-account", "disabled-user", true).await;
        insert_account(&pool, "disabled-account", "active-user", false).await;

        let accounts = list_enabled_accounts(&pool)
            .await
            .expect("enabled accounts should load");

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "active-account");
    }

    #[tokio::test]
    async fn update_account_can_clear_nullable_profile_fields() {
        let pool = test_pool().await;
        insert_user(&pool, "active-user", true).await;
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinAccount (
                id, name, siteType, baseUrl, userId, ownerId, authType,
                customCheckinUrl, enabled, retryEnabled, note, createdAt, updatedAt
             ) VALUES (
                'account-with-optionals', 'account', 'new-api', 'https://example.com',
                'user-42', 'active-user', 'access_token', '/api/checkin', 1, 1,
                'ops note', ?, ?
             )",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("account should be inserted");

        let updated = update_account(
            &pool,
            "account-with-optionals",
            &UpdateAccountRequest {
                user_id: Some(None),
                custom_checkin_url: Some(None),
                note: Some(None),
                ..Default::default()
            },
        )
        .await
        .expect("account should be updated");

        assert_eq!(updated.user_id, None);
        assert_eq!(updated.custom_checkin_url, None);
        assert_eq!(updated.note, None);
        assert_eq!(updated.name, "account");
    }
}
