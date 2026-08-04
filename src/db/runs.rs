use super::types::RunFilter;
use crate::error::{AppError, Result};
use crate::models::CheckinRun;
use chrono::{DateTime, Local, TimeZone, Utc};
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

/// Find a single check-in run by id
pub async fn find_run_by_id(db: &SqlitePool, id: &str) -> Result<Option<CheckinRun>> {
    let run = sqlx::query_as::<_, CheckinRun>(
        "SELECT id, accountId, status, message, durationMs, triggeredBy, rawResponse, createdAt FROM CheckinRun WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(run)
}

/// Delete a single check-in run by id. Returns true if a row was deleted.
///
/// L1：删除后在**同一事务**内把账户的 `lastStatus/lastMessage/lastRunAt` 重算为
/// “现存最新一条记录”的值（无剩余记录则置空）。这样删除今日唯一 success 记录后，
/// `skip_reason_for_batch` 不再判 `already_succeeded_today`，调度器/批量可重签；
/// 删除失败记录也会同步放宽今日计数，两个方向保持一致。余额列不受影响。
pub async fn delete_run(db: &SqlitePool, id: &str) -> Result<bool> {
    let mut tx = db.begin().await?;

    let target = sqlx::query_as::<_, (String, DateTime<Utc>)>(
        "SELECT accountId, createdAt FROM CheckinRun WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((account_id, _)) = target else {
        tx.commit().await?;
        return Ok(false);
    };

    let deleted = sqlx::query("DELETE FROM CheckinRun WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if deleted.rows_affected() == 0 {
        tx.commit().await?;
        return Ok(false);
    }

    // 重算账户状态：取现存最新一条记录（与记录列表一致的时间序）。
    let latest = sqlx::query_as::<_, (String, Option<String>, DateTime<Utc>)>(
        "SELECT status, message, createdAt FROM CheckinRun
         WHERE accountId = ? ORDER BY createdAt DESC, id DESC LIMIT 1",
    )
    .bind(&account_id)
    .fetch_optional(&mut *tx)
    .await?;

    match latest {
        Some((status, message, created_at)) => {
            sqlx::query(
                "UPDATE CheckinAccount SET lastStatus = ?, lastMessage = ?, lastRunAt = ?, updatedAt = ? WHERE id = ?",
            )
            .bind(status)
            .bind(message)
            .bind(created_at)
            .bind(Utc::now())
            .bind(&account_id)
            .execute(&mut *tx)
            .await?;
        }
        None => {
            sqlx::query(
                "UPDATE CheckinAccount SET lastStatus = NULL, lastMessage = NULL, lastRunAt = NULL, updatedAt = ? WHERE id = ?",
            )
            .bind(Utc::now())
            .bind(&account_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupCheckinDataResult {
    pub deleted_runs: u64,
    pub reset_accounts: u64,
    pub deleted_failure_counters: u64,
}

/// Cleanup check-in history for all accounts or one owner's accounts.
/// When reset_state is enabled, the history deletion and related state reset are atomic.
///
/// 并发说明：`reset_state` 的原子重置是“尽力而为”——若某账户的签到正处于
/// “网络调用已完成、事务未提交”阶段，其状态写入会在重置提交后重写为成功，
/// 失败计数也会被通知路径重建。该窗口极小且不丢数据，故不强加“代际标记”等
/// 更严格（需要 schema 配合）的并发控制。
pub async fn cleanup_checkin_data(
    db: &SqlitePool,
    keep_latest: usize,
    owner_id: Option<&str>,
    reset_state: bool,
) -> Result<CleanupCheckinDataResult> {
    if !reset_state {
        // 纯删除：分块提交（每批 1000 条），缩短写锁持有时间。
        // 若不分批，全表删除/保留 Top-N 的写事务会长时间阻塞并发签到写库，
        // 导致“网络签到已成功、正要写库”的事务在 busy_timeout 后失败而丢失记录。
        let mut deleted_runs = 0u64;
        const BATCH_SIZE: i64 = 1000;
        loop {
            let mut conn = db.acquire().await?;
            let batch =
                delete_runs_batch(&mut conn, owner_id, keep_latest, Some(BATCH_SIZE)).await?;
            deleted_runs += batch;
            if batch < BATCH_SIZE as u64 {
                break;
            }
        }
        return Ok(CleanupCheckinDataResult {
            deleted_runs,
            reset_accounts: 0,
            deleted_failure_counters: 0,
        });
    }

    // reset_state：删除 + 账户状态重置 + 失败计数清空需要在同一事务原子提交。
    let mut tx = db.begin().await?;
    let deleted_runs = delete_runs_batch(&mut tx, owner_id, keep_latest, None).await?;

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

    tx.commit().await?;
    Ok(CleanupCheckinDataResult {
        deleted_runs,
        reset_accounts,
        deleted_failure_counters,
    })
}

/// 执行一次记录删除。`keep_latest` 按**每账户**保留最新 N 条（`ROW_NUMBER()
/// PARTITION BY accountId`），而非全库 Top-N——后者会让低活跃账户的历史被
/// 活跃账户刷屏占满配额后整批清空。`batch_limit` 用于非原子路径的分块提交。
async fn delete_runs_batch(
    conn: &mut sqlx::SqliteConnection,
    owner_id: Option<&str>,
    keep_latest: usize,
    batch_limit: Option<i64>,
) -> Result<u64> {
    // 先选待删 id 子查询，再在外层 DELETE ... WHERE id IN (子查询 LIMIT N) 分块删除。
    // 不能直接用 `DELETE ... LIMIT`：bundled SQLite 未开启 SQLITE_ENABLE_UPDATE_DELETE_LIMIT，
    // 该语法是编译期可选项，默认构建下报 "near LIMIT: syntax error"。
    // SQLite 对含 LIMIT 的 IN 子查询会物化为临时表，因此每次删除恰好 ≤ batch_limit 行。
    let id_subquery: String = match (owner_id, keep_latest) {
        (None, 0) => "SELECT id FROM CheckinRun".to_string(),
        (Some(_), 0) => {
            "SELECT id FROM CheckinRun WHERE accountId IN (SELECT id FROM CheckinAccount WHERE ownerId = ?)"
                .to_string()
        }
        (None, _) => {
            "SELECT id FROM (
                SELECT id, ROW_NUMBER() OVER (PARTITION BY accountId ORDER BY createdAt DESC, id DESC) AS rn
                FROM CheckinRun
            ) WHERE rn > ?"
                .to_string()
        }
        (Some(_), _) => {
            "SELECT id FROM (
                SELECT r.id, ROW_NUMBER() OVER (PARTITION BY r.accountId ORDER BY r.createdAt DESC, r.id DESC) AS rn
                FROM CheckinRun r
                JOIN CheckinAccount a ON r.accountId = a.id
                WHERE a.ownerId = ?
            ) WHERE rn > ?"
                .to_string()
        }
    };

    let sql = match batch_limit {
        Some(limit) => format!("DELETE FROM CheckinRun WHERE id IN ({id_subquery} LIMIT {limit})"),
        None => format!("DELETE FROM CheckinRun WHERE id IN ({id_subquery})"),
    };

    let mut query = sqlx::query(&sql);
    match (owner_id, keep_latest) {
        (Some(owner), 0) => {
            query = query.bind(owner);
        }
        (Some(owner), keep) => {
            query = query.bind(owner).bind(keep as i64);
        }
        (None, keep) if keep > 0 => {
            query = query.bind(keep as i64);
        }
        // (None, 0)：全量删除，无占位符
        (None, _) => {}
    }

    Ok(query.execute(conn).await?.rows_affected())
}

/// 本地日历日零点对应的 UTC 时间（与统计接口的日界一致）。
/// DST 跳秒日（spring-forward）午夜可能不存在，用 earliest() 回退到前一日 23:00，
/// 最多少计昨日零点的边缘记录，但不会 panic 也不会漏掉今日记录。
fn today_start_utc() -> Result<DateTime<Utc>> {
    let local_midnight = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Internal("无法计算本地日期边界".into()))?;
    Local
        .from_local_datetime(&local_midnight)
        .earliest()
        .ok_or_else(|| AppError::Internal("无法解析本地日期边界".into()))
        .map(|dt| dt.to_utc())
}

/// 今日“真实签到尝试”次数是否计入每日上限的判定。
/// 只统计 `success` / `failed`（真尝试）；`already_checked`（站点已签）与
/// `skipped` / `pending`（未发起网络请求）不消耗每日尝试预算（M6）。
pub fn is_real_attempt(status: &str) -> bool {
    status == "success" || status == "failed"
}

/// 查询单个账户今日真实尝试次数（success + failed），供执行期在单飞锁内复核每日上限。
pub async fn count_runs_today(db: &SqlitePool, account_id: &str) -> Result<i32> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM CheckinRun
         WHERE accountId = ? AND createdAt >= ?
           AND status IN ('success', 'failed')",
    )
    .bind(account_id)
    .bind(today_start_utc()?)
    .fetch_one(db)
    .await?;
    Ok(count as i32)
}

/// Batch query today's real-attempt count for selected accounts. Empty account_ids means all accounts.
/// 与 `count_runs_today` 同口径：只统计真实尝试（success/failed），
/// already_checked/skipped/pending 不计入每日上限（M6）。
pub async fn count_runs_today_for_accounts(
    db: &SqlitePool,
    account_ids: &[String],
) -> Result<std::collections::HashMap<String, i32>> {
    let today_start_utc = today_start_utc()?;
    let mut sql =
        "SELECT accountId, COUNT(*) FROM CheckinRun WHERE createdAt >= ? AND status IN ('success', 'failed')"
            .to_string();
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
    #[tokio::test]
    async fn delete_run_removes_only_target_record() {
        let pool = pool_with_account().await;
        let now = Utc::now();
        for (run_id, account_id) in [("run-keep", "acc-1"), ("run-del", "acc-1")] {
            sqlx::query(
                "INSERT INTO CheckinRun (id, accountId, status, triggeredBy, createdAt)
                 VALUES (?, ?, 'success', 'manual', ?)",
            )
            .bind(run_id)
            .bind(account_id)
            .bind(now)
            .execute(&pool)
            .await
            .expect("run should be inserted");
        }

        let deleted = delete_run(&pool, "run-del")
            .await
            .expect("delete should succeed");
        assert!(deleted);
        assert_eq!(count_runs(&pool, "acc-1").await, 1);

        let missing = delete_run(&pool, "run-missing")
            .await
            .expect("delete missing should succeed");
        assert!(!missing);

        let found = find_run_by_id(&pool, "run-keep")
            .await
            .expect("find should succeed");
        assert!(found.is_some());
        let gone = find_run_by_id(&pool, "run-del")
            .await
            .expect("find should succeed");
        assert!(gone.is_none());
    }

    #[test]
    fn real_attempt_predicate_matches_count_semantics() {
        assert!(is_real_attempt("success"));
        assert!(is_real_attempt("failed"));
        assert!(!is_real_attempt("already_checked"));
        assert!(!is_real_attempt("skipped"));
        assert!(!is_real_attempt("pending"));
    }

    #[tokio::test]
    async fn today_count_only_counts_real_attempts() {
        let pool = pool_with_account().await;
        for (run_id, status) in [
            ("run-success", "success"),
            ("run-failed", "failed"),
            ("run-already", "already_checked"),
        ] {
            sqlx::query(
                "INSERT INTO CheckinRun (id, accountId, status, triggeredBy, createdAt)
                 VALUES (?, 'acc-1', ?, 'manual', ?)",
            )
            .bind(run_id)
            .bind(status)
            .bind(Utc::now())
            .execute(&pool)
            .await
            .expect("run should be inserted");
        }

        // 只有 success + failed 计入每日上限；already_checked 不计
        let count = count_runs_today(&pool, "acc-1")
            .await
            .expect("count should succeed");
        assert_eq!(count, 2);

        let batch = count_runs_today_for_accounts(&pool, &["acc-1".to_string()])
            .await
            .expect("batch count should succeed");
        assert_eq!(batch.get("acc-1").copied().unwrap_or(0), 2);
    }

    #[tokio::test]
    async fn delete_run_clears_account_state_when_last_record_removed() {
        let pool = pool_with_account().await;
        // 创建今日唯一 success 记录，账户状态随之同步为 success
        let run = create_run_with_status_update(
            &pool,
            "acc-1",
            "success",
            Some("ok"),
            Some(5),
            "manual",
            None,
        )
        .await
        .expect("run should be created");

        let deleted = delete_run(&pool, &run.id)
            .await
            .expect("delete should succeed");
        assert!(deleted);

        // L1：删除今日唯一 success 记录后，账户状态被清空，调度器不再判 already_succeeded_today
        let (status, msg, run_at): (Option<String>, Option<String>, Option<DateTime<Utc>>) =
            sqlx::query_as(
                "SELECT lastStatus, lastMessage, lastRunAt FROM CheckinAccount WHERE id = 'acc-1'",
            )
            .fetch_one(&pool)
            .await
            .expect("account should remain");
        assert!(status.is_none() && msg.is_none() && run_at.is_none());
    }

    #[tokio::test]
    async fn delete_run_reverts_account_state_to_previous_record() {
        let pool = pool_with_account().await;
        // 先建较旧 success 记录，再建较新 failed 记录；账户状态为 failed（最新）
        let old_run = create_run_with_status_update(
            &pool,
            "acc-1",
            "success",
            Some("ok"),
            Some(5),
            "manual",
            None,
        )
        .await
        .expect("older run should be created");
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let new_run = create_run_with_status_update(
            &pool,
            "acc-1",
            "failed",
            Some("timeout"),
            Some(8),
            "manual",
            None,
        )
        .await
        .expect("newer run should be created");

        // 删除最新 failed 记录后，账户状态应回退到较旧那条 success
        let deleted = delete_run(&pool, &new_run.id)
            .await
            .expect("delete should succeed");
        assert!(deleted);

        let (status,): (Option<String>,) =
            sqlx::query_as("SELECT lastStatus FROM CheckinAccount WHERE id = 'acc-1'")
                .fetch_one(&pool)
                .await
                .expect("account should remain");
        assert_eq!(status.as_deref(), Some("success"));
        assert!(find_run_by_id(&pool, &old_run.id)
            .await
            .expect("find should succeed")
            .is_some());
    }
}
