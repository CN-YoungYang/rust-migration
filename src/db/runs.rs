use super::types::RunFilter;
use crate::error::{AppError, Result};
use crate::models::CheckinRun;
use chrono::{Local, TimeZone, Utc};
use sqlx::SqlitePool;

/// Column list for run queries (excludes rawResponse to reduce I/O)
const RUN_LIST_COLUMNS: &str = "\
    id, accountId, status, message, durationMs, triggeredBy, \
    NULL as rawResponse, createdAt";

/// List check-in runs with filters and pagination
pub async fn list_runs_filtered(db: &SqlitePool, filter: &RunFilter) -> Result<Vec<CheckinRun>> {
    // When filtering by owner_id, JOIN CheckinAccount table
    let need_join = filter.owner_id.is_some();
    let prefix = if need_join { "r." } else { "" };

    let mut sql = if need_join {
        format!(
            "SELECT {} FROM CheckinRun r JOIN CheckinAccount a ON r.accountId = a.id WHERE 1=1",
            RUN_LIST_COLUMNS
                .replace("id,", "r.id,")
                .replace("accountId,", "r.accountId,")
                .replace("createdAt", "r.createdAt")
        )
    } else {
        format!("SELECT {} FROM CheckinRun WHERE 1=1", RUN_LIST_COLUMNS)
    };

    // Build WHERE conditions dynamically
    if filter.owner_id.is_some() {
        sql.push_str(" AND a.ownerId = ?");
    }
    if filter.account_id.is_some() {
        sql.push_str(&format!(" AND {}accountId = ?", prefix));
    }
    if filter.status.is_some() {
        sql.push_str(&format!(" AND {}status = ?", prefix));
    }
    if filter.triggered_by.is_some() {
        sql.push_str(&format!(" AND {}triggeredBy = ?", prefix));
    }
    if filter.start_date.is_some() {
        sql.push_str(&format!(" AND {}createdAt >= ?", prefix));
    }
    if filter.end_date.is_some() {
        sql.push_str(&format!(" AND {}createdAt <= ?", prefix));
    }

    sql.push_str(&format!(
        " ORDER BY {}createdAt DESC LIMIT ? OFFSET ?",
        prefix
    ));

    // Bind parameters in order
    let mut query = sqlx::query_as::<_, CheckinRun>(&sql);
    if let Some(ref oid) = filter.owner_id {
        query = query.bind(oid);
    }
    if let Some(ref aid) = filter.account_id {
        query = query.bind(aid);
    }
    if let Some(ref s) = filter.status {
        query = query.bind(s);
    }
    if let Some(ref tb) = filter.triggered_by {
        query = query.bind(tb);
    }
    if let Some(ref sd) = filter.start_date {
        query = query.bind(sd);
    }
    if let Some(ref ed) = filter.end_date {
        query = query.bind(ed);
    }
    query = query.bind(filter.limit).bind(filter.offset);

    let runs = query.fetch_all(db).await?;
    Ok(runs)
}

/// Create a check-in run record
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

/// Atomic operation: update account status and create run record in same transaction.
/// Prevents data inconsistency when update_account_status succeeds but create_run fails.
pub async fn create_run_with_status_update(
    db: &SqlitePool,
    account_id: &str,
    status: &str,
    message: Option<&str>,
    duration_ms: Option<i64>,
    triggered_by: &str,
    raw_response: Option<&str>,
) -> Result<CheckinRun> {
    let mut tx = db.begin().await?;
    let now = Utc::now();

    // 1. Update account status
    sqlx::query(
        "UPDATE CheckinAccount SET lastStatus = ?, lastMessage = ?, lastRunAt = ?, updatedAt = ? WHERE id = ?"
    )
    .bind(status)
    .bind(message)
    .bind(now)
    .bind(now)
    .bind(account_id)
    .execute(&mut *tx)
    .await?;

    // 2. Create run record
    let run_id = uuid::Uuid::new_v4().to_string();
    let run = sqlx::query_as::<_, CheckinRun>(
        "INSERT INTO CheckinRun (id, accountId, status, message, durationMs, triggeredBy, rawResponse, createdAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(&run_id)
    .bind(account_id)
    .bind(status)
    .bind(message)
    .bind(duration_ms)
    .bind(triggered_by)
    .bind(raw_response)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(run)
}

/// 与 `create_run_with_status_update` 相同，但把余额刷新（若成功）一并写入同一事务，
/// 避免"余额已更新但签到记录未创建"或反之的部分写入。
/// `balance` 为 `None` 表示未刷新余额或刷新失败，跳过余额列更新。
#[allow(clippy::too_many_arguments)] // 写入字段天然成组，拆结构体反而降低可读性
pub async fn create_run_with_status_update_and_balance(
    db: &SqlitePool,
    account_id: &str,
    status: &str,
    message: Option<&str>,
    duration_ms: Option<i64>,
    triggered_by: &str,
    raw_response: Option<&str>,
    balance: Option<f64>,
) -> Result<CheckinRun> {
    let mut tx = db.begin().await?;
    let now = Utc::now();

    // 1. 更新账户状态与余额在单条 UPDATE 中完成，保证原子
    let bal;
    let bal_at;
    let last_status = status;
    match balance {
        Some(v) => {
            bal = v;
            bal_at = now;
            sqlx::query(
                "UPDATE CheckinAccount SET lastStatus = ?, lastMessage = ?, lastRunAt = ?, lastBalance = ?, lastBalanceAt = ?, updatedAt = ? WHERE id = ?",
            )
            .bind(last_status)
            .bind(message)
            .bind(now)
            .bind(bal)
            .bind(bal_at)
            .bind(now)
            .bind(account_id)
            .execute(&mut *tx)
            .await?;
        }
        None => {
            sqlx::query(
                "UPDATE CheckinAccount SET lastStatus = ?, lastMessage = ?, lastRunAt = ?, updatedAt = ? WHERE id = ?",
            )
            .bind(last_status)
            .bind(message)
            .bind(now)
            .bind(now)
            .bind(account_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    // 2. 创建签到记录
    let run_id = uuid::Uuid::new_v4().to_string();
    let run = sqlx::query_as::<_, CheckinRun>(
        "INSERT INTO CheckinRun (id, accountId, status, message, durationMs, triggeredBy, rawResponse, createdAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(&run_id)
    .bind(account_id)
    .bind(status)
    .bind(message)
    .bind(duration_ms)
    .bind(triggered_by)
    .bind(raw_response)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(run)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupCheckinDataResult {
    pub deleted_runs: u64,
    pub reset_accounts: u64,
    pub deleted_failure_counters: u64,
}

/// Cleanup check-in history for all accounts or one owner's accounts.
/// When reset_state is enabled, the history deletion and related state reset are atomic.
pub async fn cleanup_checkin_data(
    db: &SqlitePool,
    keep_latest: usize,
    owner_id: Option<&str>,
    reset_state: bool,
) -> Result<CleanupCheckinDataResult> {
    let mut tx = db.begin().await?;

    let deleted_runs = match (owner_id, keep_latest) {
        (None, 0) => sqlx::query("DELETE FROM CheckinRun")
            .execute(&mut *tx)
            .await?
            .rows_affected(),
        (None, keep) => sqlx::query(
            "DELETE FROM CheckinRun
             WHERE id NOT IN (
                SELECT id FROM CheckinRun ORDER BY createdAt DESC, id DESC LIMIT ?
             )",
        )
        .bind(keep as i64)
        .execute(&mut *tx)
        .await?
        .rows_affected(),
        (Some(owner), 0) => sqlx::query(
            "DELETE FROM CheckinRun
             WHERE accountId IN (SELECT id FROM CheckinAccount WHERE ownerId = ?)",
        )
        .bind(owner)
        .execute(&mut *tx)
        .await?
        .rows_affected(),
        (Some(owner), keep) => sqlx::query(
            "DELETE FROM CheckinRun
             WHERE accountId IN (SELECT id FROM CheckinAccount WHERE ownerId = ?)
               AND id NOT IN (
                   SELECT r.id FROM CheckinRun r
                   JOIN CheckinAccount a ON r.accountId = a.id
                   WHERE a.ownerId = ?
                   ORDER BY r.createdAt DESC, r.id DESC
                   LIMIT ?
               )",
        )
        .bind(owner)
        .bind(owner)
        .bind(keep as i64)
        .execute(&mut *tx)
        .await?
        .rows_affected(),
    };

    let (reset_accounts, deleted_failure_counters) = if reset_state {
        let reset_accounts = match owner_id {
            None => sqlx::query(
                "UPDATE CheckinAccount
                 SET lastStatus = NULL, lastMessage = NULL, lastRunAt = NULL, updatedAt = ?
                 WHERE lastStatus IS NOT NULL OR lastMessage IS NOT NULL OR lastRunAt IS NOT NULL",
            )
            .bind(Utc::now())
            .execute(&mut *tx)
            .await?
            .rows_affected(),
            Some(owner) => sqlx::query(
                "UPDATE CheckinAccount
                 SET lastStatus = NULL, lastMessage = NULL, lastRunAt = NULL, updatedAt = ?
                 WHERE ownerId = ?
                   AND (lastStatus IS NOT NULL OR lastMessage IS NOT NULL OR lastRunAt IS NOT NULL)",
            )
            .bind(Utc::now())
            .bind(owner)
            .execute(&mut *tx)
            .await?
            .rows_affected(),
        };
        let deleted_failure_counters = match owner_id {
            None => sqlx::query("DELETE FROM FailureCounter")
                .execute(&mut *tx)
                .await?
                .rows_affected(),
            Some(owner) => sqlx::query(
                "DELETE FROM FailureCounter
                 WHERE accountId IN (SELECT id FROM CheckinAccount WHERE ownerId = ?)",
            )
            .bind(owner)
            .execute(&mut *tx)
            .await?
            .rows_affected(),
        };
        (reset_accounts, deleted_failure_counters)
    } else {
        (0, 0)
    };

    tx.commit().await?;
    Ok(CleanupCheckinDataResult {
        deleted_runs,
        reset_accounts,
        deleted_failure_counters,
    })
}

/// Cleanup old check-in runs globally, keeping only the latest N records.
pub async fn cleanup_checkin_runs(db: &SqlitePool, keep_latest: usize) -> Result<u64> {
    Ok(cleanup_checkin_data(db, keep_latest, None, false)
        .await?
        .deleted_runs)
}

/// Batch query today's run count per account, returns accountId -> count mapping.
/// More efficient than per-account COUNT (single SQL replaces N queries).
pub async fn count_runs_today_batch(
    db: &SqlitePool,
) -> Result<std::collections::HashMap<String, i32>> {
    count_runs_today_for_accounts(db, &[]).await
}

/// Batch query today's run count for selected accounts. Empty account_ids means all accounts.
pub async fn count_runs_today_for_accounts(
    db: &SqlitePool,
    account_ids: &[String],
) -> Result<std::collections::HashMap<String, i32>> {
    let local_midnight = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Internal("无法计算本地日期边界".into()))?;
    // DST-safe: on spring-forward, midnight may not exist, use earliest() to fallback to 23:00 previous day
    // This may count a few records from yesterday at most, but won't panic or miss today's records
    let today_start_utc = Local
        .from_local_datetime(&local_midnight)
        .earliest()
        .ok_or_else(|| AppError::Internal("无法解析本地日期边界".into()))?
        .to_utc();
    let mut sql = "SELECT accountId, COUNT(*) FROM CheckinRun WHERE createdAt >= ?".to_string();
    if !account_ids.is_empty() {
        let placeholders = account_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND accountId IN ({})", placeholders));
    }
    sql.push_str(" GROUP BY accountId");

    let mut query = sqlx::query_as::<_, (String, i64)>(&sql).bind(today_start_utc);
    for account_id in account_ids {
        query = query.bind(account_id);
    }

    let rows = query.fetch_all(db).await?;
    Ok(rows.into_iter().map(|(id, cnt)| (id, cnt as i32)).collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool_with_account() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        sqlx::query(
            "CREATE TABLE CheckinAccount (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                siteType TEXT NOT NULL,
                baseUrl TEXT NOT NULL,
                ownerId TEXT,
                authType TEXT NOT NULL,
                accessTokenEnc TEXT,
                cookieEnc TEXT,
                customCheckinUrl TEXT,
                userId TEXT,
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
        sqlx::query(
            "CREATE TABLE CheckinRun (
                id TEXT PRIMARY KEY,
                accountId TEXT NOT NULL,
                status TEXT NOT NULL,
                message TEXT,
                durationMs INTEGER,
                triggeredBy TEXT NOT NULL,
                rawResponse TEXT,
                createdAt TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("run table should be created");
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinAccount (id, name, siteType, baseUrl, ownerId, authType, enabled, retryEnabled, lastBalance, lastBalanceAt, lastStatus, lastMessage, lastRunAt, createdAt, updatedAt)
             VALUES ('acc-1', 'A', 'new-api', 'http://example.com', NULL, 'access_token', 1, 1, NULL, NULL, NULL, NULL, NULL, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("account should be inserted");
        pool
    }

    async fn read_account_balance_and_status(
        pool: &SqlitePool,
        id: &str,
    ) -> (Option<f64>, Option<String>) {
        sqlx::query_as::<_, (Option<f64>, Option<String>)>(
            "SELECT lastBalance, lastStatus FROM CheckinAccount WHERE id = ?",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read should succeed")
    }

    async fn count_runs(pool: &SqlitePool, account_id: &str) -> i64 {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM CheckinRun WHERE accountId = ?")
            .bind(account_id)
            .fetch_one(pool)
            .await
            .expect("count should succeed");
        n
    }

    #[tokio::test]
    async fn atomically_writes_balance_status_and_run() {
        let pool = pool_with_account().await;
        let run = create_run_with_status_update_and_balance(
            &pool,
            "acc-1",
            "success",
            Some("ok"),
            Some(123),
            "manual",
            None,
            Some(1.23),
        )
        .await
        .expect("atomic write should succeed");

        assert_eq!(run.status, "success");
        // 余额、状态、记录三者同时落库
        let (bal, status) = read_account_balance_and_status(&pool, "acc-1").await;
        assert_eq!(bal, Some(1.23));
        assert_eq!(status.as_deref(), Some("success"));
        assert_eq!(count_runs(&pool, "acc-1").await, 1);
    }

    #[tokio::test]
    async fn cleanup_retention_is_scoped_and_global_cleanup_does_not_reset_state() {
        let pool = pool_with_account().await;
        let older = Utc::now();
        let newer = older + chrono::Duration::seconds(1);
        sqlx::query("UPDATE CheckinAccount SET ownerId = 'user-1', lastStatus = 'failed' WHERE id = 'acc-1'")
            .execute(&pool)
            .await
            .expect("owned account should be updated");
        sqlx::query(
            "INSERT INTO CheckinAccount (
                id, name, siteType, baseUrl, ownerId, authType, enabled, retryEnabled,
                lastStatus, createdAt, updatedAt
             ) VALUES ('acc-2', 'B', 'new-api', 'http://example.net', 'user-2',
                'access_token', 1, 1, 'success', ?, ?)",
        )
        .bind(older)
        .bind(older)
        .execute(&pool)
        .await
        .expect("other account should be inserted");
        for (run_id, account_id, created_at) in [
            ("run-old", "acc-1", older),
            ("run-new", "acc-1", newer),
            ("run-other", "acc-2", older),
        ] {
            sqlx::query(
                "INSERT INTO CheckinRun (id, accountId, status, triggeredBy, createdAt)
                 VALUES (?, ?, 'failed', 'manual', ?)",
            )
            .bind(run_id)
            .bind(account_id)
            .bind(created_at)
            .execute(&pool)
            .await
            .expect("run should be inserted");
        }

        let scoped = cleanup_checkin_data(&pool, 1, Some("user-1"), false)
            .await
            .expect("scoped retention should succeed");
        assert_eq!(scoped.deleted_runs, 1);
        assert_eq!(count_runs(&pool, "acc-1").await, 1);
        assert_eq!(count_runs(&pool, "acc-2").await, 1);
        let remaining: (String,) =
            sqlx::query_as("SELECT id FROM CheckinRun WHERE accountId = 'acc-1'")
                .fetch_one(&pool)
                .await
                .expect("latest run should remain");
        assert_eq!(remaining.0, "run-new");

        let global = cleanup_checkin_data(&pool, 0, None, false)
            .await
            .expect("global cleanup should succeed");
        assert_eq!(global.deleted_runs, 2);
        assert_eq!(count_runs(&pool, "acc-1").await, 0);
        assert_eq!(count_runs(&pool, "acc-2").await, 0);
        let statuses: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT id, lastStatus FROM CheckinAccount ORDER BY id")
                .fetch_all(&pool)
                .await
                .expect("account statuses should remain");
        assert_eq!(statuses[0].1.as_deref(), Some("failed"));
        assert_eq!(statuses[1].1.as_deref(), Some("success"));
    }
    #[tokio::test]
    async fn cleanup_by_owner_resets_checkin_state_without_touching_balance_or_other_users() {
        let pool = pool_with_account().await;
        let now = Utc::now();
        sqlx::query(
            "CREATE TABLE FailureCounter (
                accountId TEXT PRIMARY KEY,
                consecutiveFailures INTEGER NOT NULL,
                lastFailedAt TEXT,
                lastNotifiedAt TEXT,
                updatedAt TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("failure counter table should be created");
        sqlx::query(
            "UPDATE CheckinAccount SET ownerId = 'user-1', lastBalance = 12.5,
             lastStatus = 'failed', lastMessage = 'timeout', lastRunAt = ? WHERE id = 'acc-1'",
        )
        .bind(now)
        .execute(&pool)
        .await
        .expect("owned account should be updated");
        sqlx::query(
            "INSERT INTO CheckinAccount (
                id, name, siteType, baseUrl, ownerId, authType, enabled, retryEnabled,
                lastBalance, lastStatus, lastMessage, lastRunAt, createdAt, updatedAt
             ) VALUES ('acc-2', 'B', 'new-api', 'http://example.net', 'user-2',
                'access_token', 1, 1, 8.0, 'success', 'ok', ?, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("other account should be inserted");
        for (run_id, account_id) in [("run-1", "acc-1"), ("run-2", "acc-2")] {
            sqlx::query(
                "INSERT INTO CheckinRun (id, accountId, status, triggeredBy, createdAt)
                 VALUES (?, ?, 'failed', 'manual', ?)",
            )
            .bind(run_id)
            .bind(account_id)
            .bind(now)
            .execute(&pool)
            .await
            .expect("run should be inserted");
            sqlx::query(
                "INSERT INTO FailureCounter (accountId, consecutiveFailures, updatedAt)
                 VALUES (?, 2, ?)",
            )
            .bind(account_id)
            .bind(now)
            .execute(&pool)
            .await
            .expect("failure counter should be inserted");
        }

        let result = cleanup_checkin_data(&pool, 0, Some("user-1"), true)
            .await
            .expect("cleanup should succeed");

        assert_eq!(result.deleted_runs, 1);
        assert_eq!(result.reset_accounts, 1);
        assert_eq!(result.deleted_failure_counters, 1);
        let account_1: (Option<f64>, Option<String>, Option<String>, Option<String>) =
            sqlx::query_as(
                "SELECT lastBalance, lastStatus, lastMessage, lastRunAt
                 FROM CheckinAccount WHERE id = 'acc-1'",
            )
            .fetch_one(&pool)
            .await
            .expect("account should remain");
        assert_eq!(account_1.0, Some(12.5));
        assert!(account_1.1.is_none() && account_1.2.is_none() && account_1.3.is_none());
        let account_2: (Option<String>,) =
            sqlx::query_as("SELECT lastStatus FROM CheckinAccount WHERE id = 'acc-2'")
                .fetch_one(&pool)
                .await
                .expect("other account should remain");
        assert_eq!(account_2.0.as_deref(), Some("success"));
        assert_eq!(count_runs(&pool, "acc-1").await, 0);
        assert_eq!(count_runs(&pool, "acc-2").await, 1);
        let counters: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM FailureCounter")
            .fetch_one(&pool)
            .await
            .expect("counter count should succeed");
        assert_eq!(counters.0, 1);
    }
    #[tokio::test]
    async fn skips_balance_column_when_none_but_still_writes_status_and_run() {
        let pool = pool_with_account().await;
        let run = create_run_with_status_update_and_balance(
            &pool,
            "acc-1",
            "failed",
            Some("余额刷新失败：timeout"),
            Some(10),
            "manual",
            None,
            None,
        )
        .await
        .expect("write should succeed");

        assert_eq!(run.status, "failed");
        let (bal, status) = read_account_balance_and_status(&pool, "acc-1").await;
        // 余额列保持 NULL（未被半写）
        assert!(bal.is_none());
        assert_eq!(status.as_deref(), Some("failed"));
        assert_eq!(count_runs(&pool, "acc-1").await, 1);
    }
}
