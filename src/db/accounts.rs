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

/// Column list for account queries that need the encrypted credential fields
/// (export/decrypt scenarios). Includes accessTokenEnc / cookieEnc.
const ACCOUNT_FULL_COLUMNS: &str = "\
    id, name, siteType, baseUrl, userId, ownerId, authType, \
    accessTokenEnc, cookieEnc, \
    customCheckinUrl, enabled, retryEnabled, note, \
    lastBalance, lastBalanceAt, lastStatus, lastMessage, lastRunAt, \
    createdAt, updatedAt";

/// List accounts with filters and pagination (excludes encrypted fields).
pub async fn list_accounts_filtered(
    db: &SqlitePool,
    filter: &AccountFilter,
) -> Result<Vec<CheckinAccount>> {
    list_accounts_with_columns(db, filter, ACCOUNT_LIST_COLUMNS).await
}

/// 与 `list_accounts_filtered` 相同，但包含加密凭证列（accessTokenEnc/cookieEnc），
/// 供导出等需要解密的场景使用。分页循环调用即可遍历全量，避免一次性拉取全部
/// 或构造超长 IN 子句（SQLite 占位符上限）。
pub async fn list_full_accounts_filtered(
    db: &SqlitePool,
    filter: &AccountFilter,
) -> Result<Vec<CheckinAccount>> {
    list_accounts_with_columns(db, filter, ACCOUNT_FULL_COLUMNS).await
}

async fn list_accounts_with_columns(
    db: &SqlitePool,
    filter: &AccountFilter,
    columns: &str,
) -> Result<Vec<CheckinAccount>> {
    let mut sql = format!("SELECT {columns} FROM CheckinAccount WHERE 1=1");

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
        sql.push_str(
            " AND (name LIKE ? ESCAPE '\\' OR baseUrl LIKE ? ESCAPE '\\' OR note LIKE ? ESCAPE '\\')",
        );
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
        // Low4：转义 LIKE 通配符（\ % _），避免用户输入被当作通配符匹配到意外记录
        let escaped = kw
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{}%", escaped);
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

/// Update account details.
///
/// L2：只更新请求中显式提供的列（None = 保持原值，不写入该列），替代
/// “先 find 再整行 UPDATE”的读改写。两个并发 PUT 各改不同字段时不会以陈旧
/// 基线互相覆盖；三态字段 `Some(None)` 清空为 NULL、`Some(Some(v))` 写入 v。
pub async fn update_account(
    db: &SqlitePool,
    id: &str,
    req: &UpdateAccountRequest,
) -> Result<CheckinAccount> {
    // 校验存在（NotFound），同时避免对不存在账户的空更新
    find_account_by_id(db, id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;

    let now = Utc::now();

    let mut sets: Vec<&str> = Vec::new();
    if req.name.is_some() {
        sets.push("name = ?");
    }
    if req.base_url.is_some() {
        sets.push("baseUrl = ?");
    }
    if req.user_id.is_some() {
        sets.push("userId = ?");
    }
    if req.access_token_enc.is_some() {
        sets.push("accessTokenEnc = ?");
    }
    if req.cookie_enc.is_some() {
        sets.push("cookieEnc = ?");
    }
    if req.custom_checkin_url.is_some() {
        sets.push("customCheckinUrl = ?");
    }
    if req.enabled.is_some() {
        sets.push("enabled = ?");
    }
    if req.retry_enabled.is_some() {
        sets.push("retryEnabled = ?");
    }
    if req.note.is_some() {
        sets.push("note = ?");
    }
    sets.push("updatedAt = ?");

    let sql = format!("UPDATE CheckinAccount SET {} WHERE id = ?", sets.join(", "));
    let mut query = sqlx::query(&sql);

    if let Some(v) = &req.name {
        query = query.bind(v);
    }
    if let Some(v) = &req.base_url {
        query = query.bind(v);
    }
    // 三态文本列：Some(None) 绑定 NULL，Some(Some(v)) 绑定 v
    if let Some(v) = &req.user_id {
        query = query.bind(v.as_deref());
    }
    if let Some(v) = &req.access_token_enc {
        query = query.bind(v.as_deref());
    }
    if let Some(v) = &req.cookie_enc {
        query = query.bind(v.as_deref());
    }
    if let Some(v) = &req.custom_checkin_url {
        query = query.bind(v.as_deref());
    }
    if let Some(v) = req.enabled {
        query = query.bind(v);
    }
    if let Some(v) = req.retry_enabled {
        query = query.bind(v);
    }
    if let Some(v) = &req.note {
        query = query.bind(v.as_deref());
    }

    query = query.bind(now);
    query = query.bind(id);

    query.execute(db).await?;

    // 更新后重新读取返回最新值（与通知配置更新一致）
    find_account_by_id(db, id)
        .await?
        .ok_or(crate::error::AppError::NotFound)
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

    #[tokio::test]
    async fn update_account_writes_only_requested_columns() {
        let pool = test_pool().await;
        insert_user(&pool, "active-user", true).await;
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinAccount (
                id, name, siteType, baseUrl, userId, ownerId, authType,
                customCheckinUrl, enabled, retryEnabled, note, createdAt, updatedAt
             ) VALUES (
                'account-partial', 'account', 'new-api', 'https://example.com',
                'user-42', 'active-user', 'access_token', '/api/checkin', 1, 1,
                'ops note', ?, ?
             )",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("account should be inserted");

        // L2：只改 name，未提供的 baseUrl / userId / note 等列必须保持原值不被覆盖
        let updated = update_account(
            &pool,
            "account-partial",
            &UpdateAccountRequest {
                name: Some("renamed".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("account should be updated");

        assert_eq!(updated.name, "renamed");
        assert_eq!(updated.base_url, "https://example.com");
        assert_eq!(updated.user_id.as_deref(), Some("user-42"));
        assert_eq!(updated.custom_checkin_url.as_deref(), Some("/api/checkin"));
        assert_eq!(updated.note.as_deref(), Some("ops note"));
        assert!(updated.enabled);
        assert!(updated.retry_enabled);
    }

    #[tokio::test]
    async fn keyword_search_escapes_like_wildcards() {
        let pool = test_pool().await;
        insert_user(&pool, "owner-1", true).await;
        let now = Utc::now();
        for (id, name) in [
            ("a1", "100%"),
            ("a2", "100X"),
            ("a3", "free_acct"),
            ("a4", "freeXacct"),
        ] {
            sqlx::query(
                "INSERT INTO CheckinAccount (
                    id, name, siteType, baseUrl, ownerId, authType,
                    enabled, retryEnabled, createdAt, updatedAt
                 ) VALUES (?, ?, 'new-api', 'https://example.com', 'owner-1', 'access_token', 1, 1, ?, ?)",
            )
            .bind(id)
            .bind(name)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .expect("account should be inserted");
        }

        // % 被转义为字面量：只命中含 % 的账户，不误匹配 100X
        let percent = list_accounts_filtered(
            &pool,
            &AccountFilter {
                owner_id: Some("owner-1".to_string()),
                keyword: Some("100%".to_string()),
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .expect("query should succeed");
        assert_eq!(percent.len(), 1);
        assert_eq!(percent[0].id, "a1");

        // _ 被转义为字面量：只命中含下划线的账户
        let underscore = list_accounts_filtered(
            &pool,
            &AccountFilter {
                owner_id: Some("owner-1".to_string()),
                keyword: Some("_".to_string()),
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .expect("query should succeed");
        assert_eq!(underscore.len(), 1);
        assert_eq!(underscore[0].id, "a3");

        // 普通关键字不受影响
        let plain = list_accounts_filtered(
            &pool,
            &AccountFilter {
                owner_id: Some("owner-1".to_string()),
                keyword: Some("free".to_string()),
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .expect("query should succeed");
        assert_eq!(plain.len(), 2);
    }
}
