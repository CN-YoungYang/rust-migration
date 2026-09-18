use crate::error::{AppError, Result};
use crate::models::{CheckinBatch, CheckinBatchItem};
use chrono::Utc;
use sqlx::{Sqlite, SqlitePool, Transaction};

/// 创建批次时写入的范围快照项。
#[derive(Debug, Clone)]
pub struct NewCheckinBatchItem {
    pub account_id: String,
    pub account_name: String,
    pub status: String,
    pub message: Option<String>,
}

/// 查询账号是否已经被其他执行中批次占用。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ActiveBatchConflict {
    #[sqlx(rename = "accountId")]
    pub account_id: String,
    #[sqlx(rename = "batchId")]
    pub batch_id: String,
    #[sqlx(rename = "batchStatus")]
    pub batch_status: String,
    #[sqlx(rename = "batchCompleted")]
    pub batch_completed: i64,
    #[sqlx(rename = "batchTotal")]
    pub batch_total: i64,
}

const BATCH_COLUMNS: &str = "id, createdBy, triggeredBy, status, total, completed, succeeded, alreadyChecked, skipped, failed, idempotencyKey, createdAt, startedAt, finishedAt";
const ITEM_COLUMNS: &str =
    "batchId, accountId, accountName, position, status, message, runId, createdAt, updatedAt";
const OWNER_ITEM_COLUMNS: &str =
    "i.batchId, i.accountId, i.accountName, i.position, i.status, i.message, i.runId, i.createdAt, i.updatedAt";

pub async fn find_checkin_batch(db: &SqlitePool, id: &str) -> Result<Option<CheckinBatch>> {
    Ok(sqlx::query_as::<_, CheckinBatch>(&format!(
        "SELECT {BATCH_COLUMNS} FROM CheckinBatch WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub async fn find_checkin_batch_by_idempotency(
    db: &SqlitePool,
    created_by: &str,
    idempotency_key: &str,
) -> Result<Option<CheckinBatch>> {
    Ok(sqlx::query_as::<_, CheckinBatch>(&format!(
        "SELECT {BATCH_COLUMNS} FROM CheckinBatch WHERE createdBy = ? AND idempotencyKey = ?"
    ))
    .bind(created_by)
    .bind(idempotency_key)
    .fetch_optional(db)
    .await?)
}

pub async fn list_checkin_batches(
    db: &SqlitePool,
    created_by: Option<&str>,
    active_only: bool,
    limit: i64,
) -> Result<Vec<CheckinBatch>> {
    let mut sql = format!("SELECT {BATCH_COLUMNS} FROM CheckinBatch WHERE 1 = 1");
    if created_by.is_some() {
        sql.push_str(" AND createdBy = ?");
    }
    if active_only {
        sql.push_str(" AND status IN ('pending', 'running')");
    }
    sql.push_str(" ORDER BY createdAt DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, CheckinBatch>(&sql);
    if let Some(created_by) = created_by {
        query = query.bind(created_by);
    }
    query = query.bind(limit.clamp(1, 100));
    Ok(query.fetch_all(db).await?)
}

/// 按签到范围中当前账号的所属用户筛选批次。
///
/// 管理员创建的批次 `createdBy` 是管理员，不能用创建人字段代替账号归属筛选。
pub async fn list_checkin_batches_for_account_owner(
    db: &SqlitePool,
    owner_id: &str,
    active_only: bool,
    limit: i64,
) -> Result<Vec<CheckinBatch>> {
    let mut sql = format!(
        "SELECT {BATCH_COLUMNS} FROM CheckinBatch b
         WHERE EXISTS (
             SELECT 1
             FROM CheckinBatchItem i
             JOIN CheckinAccount a ON a.id = i.accountId
             WHERE i.batchId = b.id AND a.ownerId = ?
         )"
    );
    if active_only {
        sql.push_str(" AND b.status IN ('pending', 'running')");
    }
    sql.push_str(" ORDER BY b.createdAt DESC LIMIT ?");

    Ok(sqlx::query_as::<_, CheckinBatch>(&sql)
        .bind(owner_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(db)
        .await?)
}

pub async fn list_checkin_batch_items(
    db: &SqlitePool,
    batch_id: &str,
) -> Result<Vec<CheckinBatchItem>> {
    Ok(sqlx::query_as::<_, CheckinBatchItem>(&format!(
        "SELECT {ITEM_COLUMNS} FROM CheckinBatchItem WHERE batchId = ? ORDER BY position ASC"
    ))
    .bind(batch_id)
    .fetch_all(db)
    .await?)
}

pub async fn list_checkin_batch_items_for_account_owner(
    db: &SqlitePool,
    batch_id: &str,
    owner_id: &str,
) -> Result<Vec<CheckinBatchItem>> {
    Ok(sqlx::query_as::<_, CheckinBatchItem>(&format!(
        "SELECT {OWNER_ITEM_COLUMNS}
         FROM CheckinBatchItem i
         JOIN CheckinAccount a ON a.id = i.accountId
         WHERE i.batchId = ? AND a.ownerId = ?
         ORDER BY i.position ASC"
    ))
    .bind(batch_id)
    .bind(owner_id)
    .fetch_all(db)
    .await?)
}

pub async fn checkin_batch_items_all_owned_by(
    db: &SqlitePool,
    batch_id: &str,
    owner_id: &str,
) -> Result<bool> {
    let (total, owned): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN a.ownerId = ? THEN 1 ELSE 0 END), 0)
         FROM CheckinBatchItem i
         LEFT JOIN CheckinAccount a ON a.id = i.accountId
         WHERE i.batchId = ?",
    )
    .bind(owner_id)
    .bind(batch_id)
    .fetch_one(db)
    .await?;
    Ok(total > 0 && total == owned)
}

/// 将批次汇总裁剪到调用者可见的签到范围。
pub fn scope_checkin_batch_to_items(
    mut batch: CheckinBatch,
    items: &[CheckinBatchItem],
) -> CheckinBatch {
    let total = items.len() as i64;
    let succeeded = items.iter().filter(|item| item.status == "success").count() as i64;
    let already_checked = items
        .iter()
        .filter(|item| item.status == "already_checked")
        .count() as i64;
    let skipped = items.iter().filter(|item| item.status == "skipped").count() as i64;
    let failed = items.iter().filter(|item| item.status == "failed").count() as i64;
    let completed = succeeded + already_checked + skipped + failed;
    let status = if completed < total {
        if items.iter().any(|item| item.status == "running") {
            "running"
        } else {
            "pending"
        }
    } else if failed == total {
        "failed"
    } else if failed > 0 {
        "partial_failed"
    } else {
        "completed"
    };

    batch.status = status.to_string();
    batch.total = total;
    batch.completed = completed;
    batch.succeeded = succeeded;
    batch.already_checked = already_checked;
    batch.skipped = skipped;
    batch.failed = failed;
    batch
}

pub async fn find_active_batch_conflicts(
    db: &SqlitePool,
    account_ids: &[String],
) -> Result<Vec<ActiveBatchConflict>> {
    if account_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = account_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT i.accountId AS accountId, b.id AS batchId, b.status AS batchStatus, \
                b.completed AS batchCompleted, b.total AS batchTotal \
         FROM CheckinBatchItem i \
         JOIN CheckinBatch b ON b.id = i.batchId \
         WHERE i.accountId IN ({placeholders}) \
           AND i.status IN ('pending', 'running') \
           AND b.status IN ('pending', 'running') \
         ORDER BY b.createdAt ASC"
    );
    let mut query = sqlx::query_as::<_, ActiveBatchConflict>(&sql);
    for account_id in account_ids {
        query = query.bind(account_id);
    }
    Ok(query.fetch_all(db).await?)
}

/// 在一个事务内保存范围快照。初始状态为 pending（有待执行项）或 completed（全部预先跳过）。
pub async fn create_checkin_batch(
    db: &SqlitePool,
    created_by: &str,
    idempotency_key: Option<&str>,
    items: &[NewCheckinBatchItem],
) -> Result<CheckinBatch> {
    if items.is_empty() {
        return Err(AppError::Validation("签到范围不能为空".into()));
    }

    let total = items.len() as i64;
    let completed = items
        .iter()
        .filter(|item| is_terminal_batch_item_status(&item.status))
        .count() as i64;
    let succeeded = items.iter().filter(|item| item.status == "success").count() as i64;
    let already_checked = items
        .iter()
        .filter(|item| item.status == "already_checked")
        .count() as i64;
    let skipped = items.iter().filter(|item| item.status == "skipped").count() as i64;
    let failed = items.iter().filter(|item| item.status == "failed").count() as i64;
    let status = if completed == total {
        if failed == total {
            "failed"
        } else if failed > 0 {
            "partial_failed"
        } else {
            "completed"
        }
    } else {
        "pending"
    };

    let now = Utc::now();
    let batch_id = uuid::Uuid::new_v4().to_string();
    let mut tx = db.begin().await?;

    sqlx::query(
        "INSERT INTO CheckinBatch \
         (id, createdBy, triggeredBy, status, total, completed, succeeded, alreadyChecked, skipped, failed, idempotencyKey, createdAt) \
         VALUES (?, ?, 'manual_batch', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&batch_id)
    .bind(created_by)
    .bind(status)
    .bind(total)
    .bind(completed)
    .bind(succeeded)
    .bind(already_checked)
    .bind(skipped)
    .bind(failed)
    .bind(idempotency_key)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    for (position, item) in items.iter().enumerate() {
        sqlx::query(
            "INSERT INTO CheckinBatchItem \
             (batchId, accountId, accountName, position, status, message, createdAt, updatedAt) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&batch_id)
        .bind(&item.account_id)
        .bind(&item.account_name)
        .bind(position as i64)
        .bind(&item.status)
        .bind(item.message.as_deref())
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    find_checkin_batch(db, &batch_id)
        .await?
        .ok_or(AppError::Internal("签到批次创建后无法读取".into()))
}

pub async fn mark_checkin_batch_started(db: &SqlitePool, batch_id: &str) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE CheckinBatch \
         SET status = 'running', startedAt = COALESCE(startedAt, ?) \
         WHERE id = ? AND status = 'pending'",
    )
    .bind(Utc::now())
    .bind(batch_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_pending_checkin_batch_items(
    db: &SqlitePool,
    batch_id: &str,
) -> Result<Vec<CheckinBatchItem>> {
    Ok(sqlx::query_as::<_, CheckinBatchItem>(&format!(
        "SELECT {ITEM_COLUMNS} FROM CheckinBatchItem WHERE batchId = ? AND status = 'pending' ORDER BY position ASC"
    ))
    .bind(batch_id)
    .fetch_all(db)
    .await?)
}

pub async fn mark_checkin_batch_item_running(
    db: &SqlitePool,
    batch_id: &str,
    account_id: &str,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE CheckinBatchItem SET status = 'running', updatedAt = ? \
         WHERE batchId = ? AND accountId = ? AND status = 'pending'",
    )
    .bind(Utc::now())
    .bind(batch_id)
    .bind(account_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// 手动恢复批次时，仅重置已经停滞一段时间的执行项。
/// 外部签到请求最长约 30 秒，给余额刷新和低配机器留出余量后使用 2 分钟阈值，
/// 避免正常执行中的账号被恢复操作重复提交。
pub async fn recover_stale_checkin_batch(db: &SqlitePool, batch_id: &str) -> Result<bool> {
    let now = Utc::now();
    let stale_before = now - chrono::Duration::minutes(2);
    let mut tx = db.begin().await?;
    let reset = sqlx::query(
        "UPDATE CheckinBatchItem
         SET status = 'pending', message = NULL, runId = NULL, updatedAt = ?
         WHERE batchId = ? AND status = 'running' AND updatedAt <= ?",
    )
    .bind(now)
    .bind(batch_id)
    .bind(stale_before)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if reset > 0 {
        sqlx::query(
            "UPDATE CheckinBatch SET status = 'pending', finishedAt = NULL
             WHERE id = ? AND status = 'running'",
        )
        .bind(batch_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(reset > 0)
}

pub async fn has_resumable_checkin_batch_items(db: &SqlitePool, batch_id: &str) -> Result<bool> {
    let (pending, running): (i64, i64) = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'running' THEN 1 ELSE 0 END), 0)
         FROM CheckinBatchItem WHERE batchId = ?",
    )
    .bind(batch_id)
    .fetch_one(db)
    .await?;
    Ok(pending > 0 && running == 0)
}

/// 写入逐账号结果并在同一事务内刷新批次汇总与终态。
pub async fn finish_checkin_batch_item(
    db: &SqlitePool,
    batch_id: &str,
    account_id: &str,
    status: &str,
    message: Option<&str>,
    run_id: Option<&str>,
) -> Result<()> {
    if !is_batch_item_status(status) || status == "pending" || status == "running" {
        return Err(AppError::Validation(format!(
            "无效的签到批次状态: {status}"
        )));
    }

    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE CheckinBatchItem SET status = ?, message = ?, runId = ?, updatedAt = ? \
         WHERE batchId = ? AND accountId = ?",
    )
    .bind(status)
    .bind(message)
    .bind(run_id)
    .bind(Utc::now())
    .bind(batch_id)
    .bind(account_id)
    .execute(&mut *tx)
    .await?;

    refresh_checkin_batch_summary_tx(&mut tx, batch_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn fail_checkin_batch(db: &SqlitePool, batch_id: &str, message: &str) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE CheckinBatchItem SET status = 'failed', message = ?, updatedAt = ? \
         WHERE batchId = ? AND status IN ('pending', 'running')",
    )
    .bind(message)
    .bind(Utc::now())
    .bind(batch_id)
    .execute(&mut *tx)
    .await?;
    refresh_checkin_batch_summary_tx(&mut tx, batch_id).await?;
    tx.commit().await?;
    Ok(())
}

async fn refresh_checkin_batch_summary_tx(
    tx: &mut Transaction<'_, Sqlite>,
    batch_id: &str,
) -> Result<()> {
    let (total,): (i64,) = sqlx::query_as("SELECT total FROM CheckinBatch WHERE id = ?")
        .bind(batch_id)
        .fetch_one(&mut **tx)
        .await?;
    let (succeeded, already_checked, skipped, failed): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
            COALESCE(SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN status = 'already_checked' THEN 1 ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) \
         FROM CheckinBatchItem WHERE batchId = ?",
    )
    .bind(batch_id)
    .fetch_one(&mut **tx)
    .await?;
    let completed = succeeded + already_checked + skipped + failed;
    let status = if completed < total {
        "running"
    } else if failed == 0 {
        "completed"
    } else if failed == total {
        "failed"
    } else {
        "partial_failed"
    };
    let finished_at = if completed >= total {
        Some(Utc::now())
    } else {
        None
    };

    sqlx::query(
        "UPDATE CheckinBatch SET status = ?, completed = ?, succeeded = ?, alreadyChecked = ?, skipped = ?, failed = ?, finishedAt = ? WHERE id = ?",
    )
    .bind(status)
    .bind(completed)
    .bind(succeeded)
    .bind(already_checked)
    .bind(skipped)
    .bind(failed)
    .bind(finished_at)
    .bind(batch_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn is_terminal_batch_item_status(status: &str) -> bool {
    matches!(status, "success" | "already_checked" | "failed" | "skipped")
}

fn is_batch_item_status(status: &str) -> bool {
    matches!(
        status,
        "pending" | "running" | "success" | "already_checked" | "failed" | "skipped"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite should connect");
        sqlx::query(
            "CREATE TABLE AppUser (id TEXT PRIMARY KEY, username TEXT NOT NULL, passwordHash TEXT NOT NULL, role TEXT NOT NULL, enabled INTEGER NOT NULL, createdAt TEXT NOT NULL, updatedAt TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .expect("user table should exist");
        sqlx::query(
            "CREATE TABLE CheckinBatch (id TEXT PRIMARY KEY, createdBy TEXT NOT NULL, triggeredBy TEXT NOT NULL, status TEXT NOT NULL, total INTEGER NOT NULL, completed INTEGER NOT NULL, succeeded INTEGER NOT NULL, alreadyChecked INTEGER NOT NULL, skipped INTEGER NOT NULL, failed INTEGER NOT NULL, idempotencyKey TEXT, createdAt TEXT NOT NULL, startedAt TEXT, finishedAt TEXT)",
        )
        .execute(&pool)
        .await
        .expect("batch table should exist");
        sqlx::query(
            "CREATE UNIQUE INDEX idx_batch_key ON CheckinBatch(createdBy, idempotencyKey) WHERE idempotencyKey IS NOT NULL",
        )
        .execute(&pool)
        .await
        .expect("batch index should exist");
        sqlx::query(
            "CREATE TABLE CheckinBatchItem (batchId TEXT NOT NULL, accountId TEXT NOT NULL, accountName TEXT NOT NULL, position INTEGER NOT NULL, status TEXT NOT NULL, message TEXT, runId TEXT, createdAt TEXT NOT NULL, updatedAt TEXT NOT NULL, PRIMARY KEY(batchId, accountId))",
        )
        .execute(&pool)
            .await
            .expect("batch item table should exist");
        sqlx::query("CREATE TABLE CheckinAccount (id TEXT PRIMARY KEY, ownerId TEXT)")
            .execute(&pool)
            .await
            .expect("account table should exist");
        sqlx::query(
            "CREATE UNIQUE INDEX idx_batch_active_account ON CheckinBatchItem(accountId) WHERE status IN ('pending', 'running')",
        )
        .execute(&pool)
        .await
        .expect("active account index should exist");
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO AppUser (id, username, passwordHash, role, enabled, createdAt, updatedAt) VALUES ('user-1', 'user', 'hash', 'USER', 1, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("user should exist");
        pool
    }

    fn items() -> Vec<NewCheckinBatchItem> {
        vec![
            NewCheckinBatchItem {
                account_id: "account-1".into(),
                account_name: "账户 1".into(),
                status: "pending".into(),
                message: None,
            },
            NewCheckinBatchItem {
                account_id: "account-2".into(),
                account_name: "账户 2".into(),
                status: "already_checked".into(),
                message: Some("今日已签到".into()),
            },
        ]
    }

    #[tokio::test]
    async fn persists_scope_and_refreshes_terminal_summary() {
        let pool = pool().await;
        let batch = create_checkin_batch(&pool, "user-1", Some("request-1"), &items())
            .await
            .expect("batch should be created");
        assert_eq!(batch.status, "pending");
        assert_eq!(batch.total, 2);
        assert_eq!(batch.completed, 1);
        assert_eq!(batch.already_checked, 1);

        mark_checkin_batch_started(&pool, &batch.id)
            .await
            .expect("batch should start");
        mark_checkin_batch_item_running(&pool, &batch.id, "account-1")
            .await
            .expect("item should start");
        finish_checkin_batch_item(
            &pool,
            &batch.id,
            "account-1",
            "success",
            Some("签到成功"),
            Some("run-1"),
        )
        .await
        .expect("item should finish");

        let finished = find_checkin_batch(&pool, &batch.id)
            .await
            .expect("batch should be found")
            .expect("batch should exist");
        assert_eq!(finished.status, "completed");
        assert_eq!(finished.completed, 2);
        assert_eq!(finished.succeeded, 1);
        assert_eq!(finished.already_checked, 1);
        assert_eq!(
            list_checkin_batch_items(&pool, &batch.id)
                .await
                .unwrap()
                .len(),
            2
        );
        let visible = list_checkin_batch_items_for_account_owner(&pool, &batch.id, "owner-1")
            .await
            .unwrap();
        assert_eq!(visible.len(), 0);

        let scoped = scope_checkin_batch_to_items(
            finished,
            &[CheckinBatchItem {
                batch_id: batch.id,
                account_id: "account-1".into(),
                account_name: "账户 1".into(),
                position: 0,
                status: "success".into(),
                message: None,
                run_id: Some("run-1".into()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
        );
        assert_eq!(scoped.total, 1);
        assert_eq!(scoped.completed, 1);
        assert_eq!(scoped.succeeded, 1);
        assert_eq!(scoped.status, "completed");
    }

    #[tokio::test]
    async fn idempotency_key_is_scoped_to_creator() {
        let pool = pool().await;
        let batch = create_checkin_batch(&pool, "user-1", Some("request-1"), &items())
            .await
            .expect("batch should be created");
        let found = find_checkin_batch_by_idempotency(&pool, "user-1", "request-1")
            .await
            .expect("lookup should succeed")
            .expect("original batch should be returned");
        assert_eq!(found.id, batch.id);
        assert!(find_checkin_batch_by_idempotency(&pool, "user-1", "other")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn account_owner_batch_filter_does_not_use_batch_creator() {
        let pool = pool().await;
        sqlx::query("INSERT INTO CheckinAccount (id, ownerId) VALUES ('account-1', 'owner-1'), ('account-2', 'owner-2')")
            .execute(&pool)
            .await
            .expect("accounts should exist");

        let owner_batch = create_checkin_batch(
            &pool,
            "user-1",
            None,
            &[NewCheckinBatchItem {
                account_id: "account-1".into(),
                account_name: "账户 1".into(),
                status: "pending".into(),
                message: None,
            }],
        )
        .await
        .expect("owner batch should be created");
        let other_batch = create_checkin_batch(
            &pool,
            "user-1",
            None,
            &[NewCheckinBatchItem {
                account_id: "account-2".into(),
                account_name: "账户 2".into(),
                status: "pending".into(),
                message: None,
            }],
        )
        .await
        .expect("other batch should be created");

        let batches = list_checkin_batches_for_account_owner(&pool, "owner-1", true, 20)
            .await
            .expect("owner filter should work");
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].id, owner_batch.id);
        assert_ne!(batches[0].id, other_batch.id);
        assert!(
            checkin_batch_items_all_owned_by(&pool, &owner_batch.id, "owner-1")
                .await
                .unwrap()
        );
        assert!(
            !checkin_batch_items_all_owned_by(&pool, &other_batch.id, "owner-1")
                .await
                .unwrap()
        );

        let mixed_batch = create_checkin_batch(
            &pool,
            "user-1",
            None,
            &[
                NewCheckinBatchItem {
                    account_id: "account-1".into(),
                    account_name: "账户 1".into(),
                    status: "success".into(),
                    message: None,
                },
                NewCheckinBatchItem {
                    account_id: "account-2".into(),
                    account_name: "账户 2".into(),
                    status: "failed".into(),
                    message: Some("失败".into()),
                },
            ],
        )
        .await
        .expect("mixed batch should be created");
        let visible_items =
            list_checkin_batch_items_for_account_owner(&pool, &mixed_batch.id, "owner-1")
                .await
                .unwrap();
        assert_eq!(visible_items.len(), 1);
        assert_eq!(visible_items[0].account_id, "account-1");
        assert!(
            !checkin_batch_items_all_owned_by(&pool, &mixed_batch.id, "owner-1")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn recovery_resets_only_stale_running_items() {
        let pool = pool().await;
        let batch = create_checkin_batch(&pool, "user-1", None, &items())
            .await
            .expect("batch should be created");
        mark_checkin_batch_started(&pool, &batch.id)
            .await
            .expect("batch should start");
        mark_checkin_batch_item_running(&pool, &batch.id, "account-1")
            .await
            .expect("item should start");
        sqlx::query(
            "UPDATE CheckinBatchItem SET updatedAt = ? WHERE batchId = ? AND accountId = ?",
        )
        .bind(Utc::now() - chrono::Duration::minutes(3))
        .bind(&batch.id)
        .bind("account-1")
        .execute(&pool)
        .await
        .expect("item should become stale");

        assert!(recover_stale_checkin_batch(&pool, &batch.id)
            .await
            .expect("recovery should succeed"));
        let item = list_checkin_batch_items(&pool, &batch.id)
            .await
            .expect("items should load")
            .into_iter()
            .find(|item| item.account_id == "account-1")
            .expect("item should exist");
        assert_eq!(item.status, "pending");
        assert_eq!(
            find_checkin_batch(&pool, &batch.id)
                .await
                .expect("batch should load")
                .expect("batch should exist")
                .status,
            "pending"
        );
    }
}
