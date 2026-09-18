use crate::error::Result;
use crate::models::CheckinAccount;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct WorkbenchAccountFilter {
    pub owner_id: Option<String>,
    pub site_type: Option<String>,
    pub enabled: Option<bool>,
    pub status: Option<String>,
    pub today_result: Option<String>,
    pub balance_warning: Option<bool>,
    pub keyword: Option<String>,
    pub today_start: DateTime<Utc>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchSummary {
    pub total_accounts: i64,
    pub enabled_accounts: i64,
    pub today_success: i64,
    pub today_already_checked: i64,
    pub today_failed: i64,
    pub today_pending: i64,
    pub balance_warnings: i64,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchFailure {
    pub id: String,
    #[sqlx(rename = "accountId")]
    pub account_id: String,
    #[sqlx(rename = "accountName")]
    pub account_name: String,
    #[sqlx(rename = "siteType")]
    pub site_type: String,
    pub status: String,
    pub message: Option<String>,
    #[sqlx(rename = "durationMs")]
    pub duration_ms: Option<i64>,
    #[sqlx(rename = "triggeredBy")]
    pub triggered_by: String,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "retryEnabled")]
    pub retry_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchScheduledRun {
    pub id: String,
    #[sqlx(rename = "accountId")]
    pub account_id: String,
    #[sqlx(rename = "accountName")]
    pub account_name: String,
    pub status: String,
    pub message: Option<String>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

const BALANCE_WARNING_EXPRESSION: &str = "EXISTS (SELECT 1 FROM NotificationConfig n WHERE n.ownerId = a.ownerId AND n.enabled = 1 AND n.onBalanceLow = 1 AND n.balanceThreshold IS NOT NULL AND a.lastBalance IS NOT NULL AND a.lastBalance < n.balanceThreshold * 500000.0)";

const ACCOUNT_LIST_COLUMNS: &str = "a.id AS id, a.name AS name, a.siteType AS siteType, a.baseUrl AS baseUrl, a.userId AS userId, a.ownerId AS ownerId, a.authType AS authType, NULL AS accessTokenEnc, NULL AS cookieEnc, a.customCheckinUrl AS customCheckinUrl, a.enabled AS enabled, a.retryEnabled AS retryEnabled, a.note AS note, a.lastBalance AS lastBalance, a.lastBalanceAt AS lastBalanceAt, a.lastStatus AS lastStatus, a.lastMessage AS lastMessage, a.lastRunAt AS lastRunAt, a.createdAt AS createdAt, a.updatedAt AS updatedAt";

fn append_account_conditions(sql: &mut String, filter: &WorkbenchAccountFilter) {
    if filter.owner_id.is_some() {
        sql.push_str(" AND a.ownerId = ?");
    }
    if filter.site_type.is_some() {
        sql.push_str(" AND a.siteType = ?");
    }
    if filter.enabled.is_some() {
        sql.push_str(" AND a.enabled = ?");
    }
    if let Some(status) = filter.status.as_deref() {
        if status == "never" {
            sql.push_str(" AND a.lastStatus IS NULL");
        } else if status == "disabled" {
            sql.push_str(" AND a.enabled = 0");
        } else {
            sql.push_str(" AND a.lastStatus = ?");
        }
    }
    if let Some(result) = filter.today_result.as_deref() {
        match result {
            "not_checked" => sql.push_str(" AND (a.lastRunAt IS NULL OR a.lastRunAt < ?)"),
            "already_checked" => {
                sql.push_str(" AND a.lastRunAt >= ? AND a.lastStatus = 'already_checked'")
            }
            "success" => sql.push_str(" AND a.lastRunAt >= ? AND a.lastStatus = 'success'"),
            "failed" => sql.push_str(" AND a.lastRunAt >= ? AND a.lastStatus = 'failed'"),
            "pending" => sql.push_str(
                " AND a.enabled = 1 AND (a.lastRunAt IS NULL OR NOT (a.lastRunAt >= ? AND COALESCE(a.lastStatus, '') IN ('success', 'already_checked', 'failed'))) ",
            ),
            _ => {}
        }
    }
    if let Some(warning) = filter.balance_warning {
        if warning {
            sql.push_str(" AND ");
            sql.push_str(BALANCE_WARNING_EXPRESSION);
        } else {
            sql.push_str(" AND NOT (");
            sql.push_str(BALANCE_WARNING_EXPRESSION);
            sql.push(')');
        }
    }
    if filter.keyword.is_some() {
        sql.push_str(
            " AND (a.name LIKE ? ESCAPE '\\' OR a.baseUrl LIKE ? ESCAPE '\\' OR a.note LIKE ? ESCAPE '\\')",
        );
    }
}

/// 给 query_as/query_scalar 共用的绑定宏。使用拥有所有权的字符串，避免把 SQL
/// 字符串的生命周期和筛选器借用生命周期绑在一起。
macro_rules! bind_account_conditions {
    ($query:expr, $filter:expr) => {{
        let mut query = $query;
        if let Some(owner_id) = $filter.owner_id.as_deref() {
            query = query.bind(owner_id.to_owned());
        }
        if let Some(site_type) = $filter.site_type.as_deref() {
            query = query.bind(site_type.to_owned());
        }
        if let Some(enabled) = $filter.enabled {
            query = query.bind(enabled);
        }
        if let Some(status) = $filter.status.as_deref() {
            if status != "never" && status != "disabled" {
                query = query.bind(status.to_owned());
            }
        }
        if let Some(result) = $filter.today_result.as_deref() {
            if matches!(
                result,
                "not_checked" | "already_checked" | "success" | "failed" | "pending"
            ) {
                query = query.bind($filter.today_start);
            }
        }
        if let Some(keyword) = $filter.keyword.as_deref() {
            let escaped = keyword
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            let pattern = format!("%{escaped}%");
            query = query
                .bind(pattern.clone())
                .bind(pattern.clone())
                .bind(pattern);
        }
        query
    }};
}

pub async fn list_workbench_accounts(
    db: &SqlitePool,
    filter: &WorkbenchAccountFilter,
) -> Result<Vec<CheckinAccount>> {
    let mut sql = format!("SELECT {ACCOUNT_LIST_COLUMNS} FROM CheckinAccount a WHERE 1 = 1");
    append_account_conditions(&mut sql, filter);
    sql.push_str(" ORDER BY a.createdAt DESC, a.id DESC LIMIT ? OFFSET ?");

    let query = bind_account_conditions!(sqlx::query_as::<_, CheckinAccount>(&sql), filter)
        .bind(filter.limit.clamp(1, 100))
        .bind(filter.offset.max(0));
    Ok(query.fetch_all(db).await?)
}

pub async fn count_workbench_accounts(
    db: &SqlitePool,
    filter: &WorkbenchAccountFilter,
) -> Result<i64> {
    let mut sql = "SELECT COUNT(*) FROM CheckinAccount a WHERE 1 = 1".to_string();
    append_account_conditions(&mut sql, filter);
    let query = bind_account_conditions!(sqlx::query_scalar::<_, i64>(&sql), filter);
    Ok(query.fetch_one(db).await?)
}

pub async fn list_balance_warning_account_ids(
    db: &SqlitePool,
    owner_id: Option<&str>,
    account_ids: &[String],
) -> Result<HashSet<String>> {
    if account_ids.is_empty() {
        return Ok(HashSet::new());
    }
    let placeholders = account_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let mut sql = format!(
        "SELECT a.id FROM CheckinAccount a WHERE {BALANCE_WARNING_EXPRESSION} AND a.id IN ({placeholders})"
    );
    if owner_id.is_some() {
        sql.push_str(" AND a.ownerId = ?");
    }
    let mut query = sqlx::query_scalar::<_, String>(&sql);
    for account_id in account_ids {
        query = query.bind(account_id);
    }
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    Ok(query.fetch_all(db).await?.into_iter().collect())
}

pub async fn get_workbench_summary(
    db: &SqlitePool,
    owner_id: Option<&str>,
    today_start: DateTime<Utc>,
) -> Result<WorkbenchSummary> {
    let owner_filter = if owner_id.is_some() {
        " AND a.ownerId = ?"
    } else {
        ""
    };
    let sql = format!(
        "SELECT
            COUNT(*),
            COALESCE(SUM(CASE WHEN a.enabled = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN a.lastRunAt >= ? AND a.lastStatus = 'success' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN a.lastRunAt >= ? AND a.lastStatus = 'already_checked' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN a.lastRunAt >= ? AND a.lastStatus = 'failed' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN a.enabled = 1 AND (a.lastRunAt IS NULL OR NOT (a.lastRunAt >= ? AND COALESCE(a.lastStatus, '') IN ('success', 'already_checked', 'failed'))) THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN {BALANCE_WARNING_EXPRESSION} THEN 1 ELSE 0 END), 0)
         FROM CheckinAccount a WHERE 1 = 1{owner_filter}"
    );
    let mut query = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64)>(&sql)
        .bind(today_start)
        .bind(today_start)
        .bind(today_start)
        .bind(today_start);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    let (
        total_accounts,
        enabled_accounts,
        today_success,
        today_already_checked,
        today_failed,
        today_pending,
        balance_warnings,
    ) = query.fetch_one(db).await?;
    Ok(WorkbenchSummary {
        total_accounts,
        enabled_accounts,
        today_success,
        today_already_checked,
        today_failed,
        today_pending,
        balance_warnings,
    })
}

pub async fn list_workbench_failures(
    db: &SqlitePool,
    owner_id: Option<&str>,
    limit: i64,
) -> Result<Vec<WorkbenchFailure>> {
    let mut sql = "SELECT r.id AS id, r.accountId AS accountId, a.name AS accountName, a.siteType AS siteType, r.status AS status, r.message AS message, r.durationMs AS durationMs, r.triggeredBy AS triggeredBy, r.createdAt AS createdAt, a.retryEnabled AS retryEnabled FROM CheckinRun r JOIN CheckinAccount a ON a.id = r.accountId WHERE r.status = 'failed'".to_string();
    if owner_id.is_some() {
        sql.push_str(" AND a.ownerId = ?");
    }
    sql.push_str(" ORDER BY r.createdAt DESC, r.id DESC LIMIT ?");
    let mut query = sqlx::query_as::<_, WorkbenchFailure>(&sql);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    query = query.bind(limit.clamp(1, 50));
    Ok(query.fetch_all(db).await?)
}

pub async fn find_latest_scheduled_run(
    db: &SqlitePool,
    owner_id: Option<&str>,
) -> Result<Option<WorkbenchScheduledRun>> {
    let mut sql = "SELECT r.id AS id, r.accountId AS accountId, a.name AS accountName, r.status AS status, r.message AS message, r.createdAt AS createdAt FROM CheckinRun r JOIN CheckinAccount a ON a.id = r.accountId WHERE r.triggeredBy = 'scheduled'".to_string();
    if owner_id.is_some() {
        sql.push_str(" AND a.ownerId = ?");
    }
    sql.push_str(" ORDER BY r.createdAt DESC, r.id DESC LIMIT 1");
    let mut query = sqlx::query_as::<_, WorkbenchScheduledRun>(&sql);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    Ok(query.fetch_optional(db).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::business_time;
    use chrono::{Duration, Utc};
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite should connect");
        sqlx::query(include_str!("../../migrations/20260611_init.sql"))
            .execute(&pool)
            .await
            .expect("migration should execute");
        pool
    }

    async fn insert_user(db: &SqlitePool, id: &str) {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO AppUser (id, username, passwordHash, role, enabled, createdAt, updatedAt) \
             VALUES (?, ?, 'hash', 'USER', 1, ?, ?)",
        )
        .bind(id)
        .bind(id)
        .bind(now)
        .bind(now)
        .execute(db)
        .await
        .expect("user should insert");
    }

    async fn insert_account(
        db: &SqlitePool,
        id: &str,
        owner_id: &str,
        enabled: bool,
        status: Option<&str>,
        last_run_at: Option<chrono::DateTime<Utc>>,
        balance: Option<f64>,
    ) {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinAccount \
             (id, name, siteType, baseUrl, ownerId, authType, enabled, retryEnabled, lastBalance, lastStatus, lastRunAt, createdAt, updatedAt) \
             VALUES (?, ?, 'new-api', 'https://example.com', ?, 'access_token', ?, 1, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(id)
        .bind(owner_id)
        .bind(enabled)
        .bind(balance)
        .bind(status)
        .bind(last_run_at)
        .bind(now)
        .bind(now)
        .execute(db)
        .await
        .expect("account should insert");
    }

    #[test]
    fn workbench_filter_includes_today_and_balance_dimensions() {
        let mut sql = "SELECT * FROM CheckinAccount a WHERE 1 = 1".to_string();
        append_account_conditions(
            &mut sql,
            &WorkbenchAccountFilter {
                today_result: Some("already_checked".into()),
                balance_warning: Some(true),
                today_start: Utc::now(),
                ..Default::default()
            },
        );
        assert!(sql.contains("lastStatus = 'already_checked'"));
        assert!(sql.contains("NotificationConfig"));
    }

    #[test]
    fn workbench_filter_treats_disabled_as_a_status_without_an_extra_bind() {
        let mut sql = "SELECT * FROM CheckinAccount a WHERE 1 = 1".to_string();
        append_account_conditions(
            &mut sql,
            &WorkbenchAccountFilter {
                status: Some("disabled".into()),
                ..Default::default()
            },
        );
        assert!(sql.contains("a.enabled = 0"));
        assert!(!sql.ends_with("?"));
    }

    #[test]
    fn pending_filter_includes_unchecked_and_currently_pending_accounts() {
        let mut sql = "SELECT * FROM CheckinAccount a WHERE 1 = 1".to_string();
        append_account_conditions(
            &mut sql,
            &WorkbenchAccountFilter {
                today_result: Some("pending".into()),
                today_start: Utc::now(),
                ..Default::default()
            },
        );
        assert!(sql.contains("a.enabled = 1"));
        assert!(sql.contains("a.lastRunAt IS NULL OR NOT"));
        assert!(
            sql.contains("COALESCE(a.lastStatus, '') IN ('success', 'already_checked', 'failed')")
        );
    }

    #[tokio::test]
    async fn aggregates_today_data_and_balance_warnings_by_owner() {
        let db = setup_db().await;
        insert_user(&db, "user-1").await;
        insert_user(&db, "user-2").await;

        let today_start = business_time::day_start_utc(business_time::today()).unwrap();
        insert_account(
            &db,
            "success-1",
            "user-1",
            true,
            Some("success"),
            Some(today_start + Duration::hours(1)),
            Some(1_000_000.0),
        )
        .await;
        insert_account(
            &db,
            "failed-1",
            "user-1",
            true,
            Some("failed"),
            Some(today_start + Duration::hours(2)),
            Some(100_000.0),
        )
        .await;
        insert_account(
            &db,
            "pending-1",
            "user-1",
            true,
            Some("success"),
            Some(today_start - Duration::hours(1)),
            None,
        )
        .await;
        insert_account(&db, "disabled-1", "user-1", false, None, None, None).await;
        insert_account(
            &db,
            "peer-1",
            "user-2",
            true,
            Some("success"),
            Some(today_start + Duration::hours(1)),
            Some(100_000.0),
        )
        .await;

        let now = Utc::now();
        sqlx::query(
            "INSERT INTO NotificationConfig (id, ownerId, notifyType, enabled, onFailure, failureThreshold, onBalanceLow, balanceThreshold, createdAt, updatedAt) \
             VALUES ('low-balance-1', 'user-1', 'webhook', 1, 0, 1, 1, 1.0, ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&db)
        .await
        .expect("notification should insert");

        let summary = get_workbench_summary(&db, Some("user-1"), today_start)
            .await
            .unwrap();
        assert_eq!(summary.total_accounts, 4);
        assert_eq!(summary.enabled_accounts, 3);
        assert_eq!(summary.today_success, 1);
        assert_eq!(summary.today_failed, 1);
        assert_eq!(summary.today_pending, 1);
        assert_eq!(summary.balance_warnings, 1);

        let warning_filter = WorkbenchAccountFilter {
            owner_id: Some("user-1".into()),
            balance_warning: Some(true),
            today_start,
            limit: 50,
            ..Default::default()
        };
        let warnings = list_workbench_accounts(&db, &warning_filter).await.unwrap();
        assert_eq!(
            warnings
                .iter()
                .map(|account| account.id.as_str())
                .collect::<Vec<_>>(),
            vec!["failed-1"]
        );
        assert_eq!(
            count_workbench_accounts(&db, &warning_filter)
                .await
                .unwrap(),
            1
        );

        let peer_summary = get_workbench_summary(&db, Some("user-2"), today_start)
            .await
            .unwrap();
        assert_eq!(peer_summary.total_accounts, 1);
        assert_eq!(peer_summary.balance_warnings, 0);
    }
}
