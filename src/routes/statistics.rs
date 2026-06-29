use crate::{
    error::{AppError, Result},
    AppState,
};
use axum::{
    extract::{Query, State},
    response::Json,
    Extension,
};
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct StatisticsQuery {
    /// 开始日期 (YYYY-MM-DD)，默认为 30 天前
    #[serde(rename = "startDate")]
    start_date: Option<String>,
    /// 结束日期 (YYYY-MM-DD)，默认为今天
    #[serde(rename = "endDate")]
    end_date: Option<String>,
    /// 管理员查看指定用户（AppUser.id）的统计数据
    #[serde(rename = "userId")]
    user_id: Option<String>,
}

#[derive(Serialize)]
pub struct StatisticsResponse {
    /// 概览数据
    overview: Overview,
    /// 每日签到趋势
    #[serde(rename = "dailyTrend")]
    daily_trend: Vec<DailyStats>,
    /// 站点统计
    #[serde(rename = "siteStats")]
    site_stats: Vec<SiteStats>,
    /// 余额变化趋势（最近30天）
    #[serde(rename = "balanceTrend")]
    balance_trend: Vec<BalanceTrend>,
    /// 最近失败记录
    #[serde(rename = "recentFailures")]
    recent_failures: Vec<RecentFailure>,
}

#[derive(Serialize)]
pub struct Overview {
    /// 总账户数
    #[serde(rename = "totalAccounts")]
    total_accounts: i64,
    /// 已启用账户数
    #[serde(rename = "enabledAccounts")]
    enabled_accounts: i64,
    /// 今日签到成功数
    #[serde(rename = "todaySuccess")]
    today_success: i64,
    /// 今日签到失败数
    #[serde(rename = "todayFailed")]
    today_failed: i64,
    /// 总签到次数（时间范围内）
    #[serde(rename = "totalRuns")]
    total_runs: i64,
    /// 签到成功率（时间范围内）
    #[serde(rename = "successRate")]
    success_rate: f64,
    /// 总余额（美元）
    #[serde(rename = "totalBalance")]
    total_balance: f64,
}

#[derive(Serialize)]
pub struct DailyStats {
    /// 日期 (YYYY-MM-DD)
    date: String,
    /// 成功次数
    success: i64,
    /// 失败次数
    failed: i64,
    /// 已签到次数（包含 already_checked）
    #[serde(rename = "alreadyChecked")]
    already_checked: i64,
    /// 总次数
    total: i64,
    /// 成功率
    #[serde(rename = "successRate")]
    success_rate: f64,
}

#[derive(Serialize)]
pub struct SiteStats {
    /// 站点类型
    #[serde(rename = "siteType")]
    site_type: String,
    /// 账户数量
    #[serde(rename = "accountCount")]
    account_count: i64,
    /// 总签到次数
    #[serde(rename = "totalRuns")]
    total_runs: i64,
    /// 成功次数
    success: i64,
    /// 失败次数
    failed: i64,
    /// 成功率
    #[serde(rename = "successRate")]
    success_rate: f64,
    /// 平均响应时间（毫秒）
    #[serde(rename = "avgDuration")]
    avg_duration: f64,
}

#[derive(Serialize)]
pub struct BalanceTrend {
    /// 日期 (YYYY-MM-DD)
    date: String,
    /// 总余额（美元，500000 quota = $1）
    balance: f64,
}

#[derive(Serialize)]
pub struct RecentFailure {
    #[serde(rename = "runId")]
    run_id: String,
    #[serde(rename = "accountId")]
    account_id: String,
    #[serde(rename = "accountName")]
    account_name: String,
    #[serde(rename = "siteType")]
    site_type: String,
    #[serde(rename = "ownerName")]
    owner_name: Option<String>,
    message: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: DateTime<Utc>,
}

fn local_day_start(date: NaiveDate) -> Result<DateTime<Utc>> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Internal("无法计算本地日期开始时间".into()))?;
    naive
        .and_local_timezone(Local)
        .earliest()
        .map(|dt| dt.to_utc())
        .ok_or_else(|| AppError::Internal("无法解析本地日期开始时间".into()))
}

fn local_day_end(date: NaiveDate) -> Result<DateTime<Utc>> {
    let naive = date
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| AppError::Internal("无法计算本地日期结束时间".into()))?;
    naive
        .and_local_timezone(Local)
        .latest()
        .map(|dt| dt.to_utc())
        .ok_or_else(|| AppError::Internal("无法解析本地日期结束时间".into()))
}

fn resolve_owner_filter(
    user: &crate::models::AppUser,
    is_admin: bool,
    requested_user_id: Option<&str>,
) -> Result<Option<String>> {
    let requested_user_id = requested_user_id.map(str::trim).filter(|id| !id.is_empty());

    if is_admin {
        return Ok(requested_user_id.map(ToOwned::to_owned));
    }

    if let Some(owner_id) = requested_user_id {
        if owner_id != user.id {
            return Err(AppError::Forbidden);
        }
    }

    Ok(Some(user.id.clone()))
}

/// GET /api/statistics - 获取统计数据
pub async fn get_statistics(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    Query(query): Query<StatisticsQuery>,
) -> Result<Json<serde_json::Value>> {
    let is_admin = user.role == "ADMIN" || user.role == "SUPER_ADMIN";

    // 解析日期范围，默认最近 30 天
    let end_date = match &query.end_date {
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| crate::error::AppError::Validation("无效的结束日期格式".into()))?,
        None => Local::now().date_naive(),
    };

    let start_date = match &query.start_date {
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| crate::error::AppError::Validation("无效的开始日期格式".into()))?,
        None => end_date - Duration::days(29), // 默认 30 天
    };
    if start_date > end_date {
        return Err(crate::error::AppError::Validation(
            "开始日期不能晚于结束日期".into(),
        ));
    }
    if (end_date - start_date).num_days() > 180 {
        return Err(crate::error::AppError::Validation(
            "统计查询范围不能超过 180 天".into(),
        ));
    }

    let start_datetime = local_day_start(start_date)?;
    let end_datetime = local_day_end(end_date)?;
    let owner_id = resolve_owner_filter(&user, is_admin, query.user_id.as_deref())?;
    let owner_id = owner_id.as_deref();

    // 计算概览数据
    let overview = calculate_overview(&state.db, owner_id, start_datetime, end_datetime).await?;

    // 计算每日趋势
    let daily_trend =
        calculate_daily_trend(&state.db, owner_id, start_datetime, end_datetime).await?;

    // 计算站点统计
    let site_stats =
        calculate_site_stats(&state.db, owner_id, start_datetime, end_datetime).await?;

    // 计算余额趋势（最近30天）
    let balance_trend = calculate_balance_trend(&state.db, owner_id).await?;

    let recent_failures = calculate_recent_failures(&state.db, owner_id).await?;

    Ok(crate::routes::data(StatisticsResponse {
        overview,
        daily_trend,
        site_stats,
        balance_trend,
        recent_failures,
    }))
}

async fn calculate_overview(
    db: &sqlx::SqlitePool,
    owner_id: Option<&str>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Overview> {
    let owner_filter = if owner_id.is_some() {
        " AND ownerId = ?"
    } else {
        ""
    };

    // 总账户数和已启用账户数
    let sql = format!("SELECT COUNT(*) as total, SUM(CASE WHEN enabled = 1 THEN 1 ELSE 0 END) as enabled FROM CheckinAccount WHERE 1=1{}", owner_filter);
    let mut query = sqlx::query_as::<_, (i64, Option<i64>)>(&sql);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    let (total_accounts, enabled_accounts) = query.fetch_one(db).await?;
    let enabled_accounts = enabled_accounts.unwrap_or(0);

    // 今日签到统计
    let today_start = local_day_start(Local::now().date_naive())?;

    let sql = format!(
        "SELECT
            SUM(CASE WHEN status IN ('success', 'already_checked') THEN 1 ELSE 0 END) as success,
            SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed
         FROM CheckinRun cr
         JOIN CheckinAccount ca ON cr.accountId = ca.id
         WHERE cr.createdAt >= ?{}",
        if owner_id.is_some() {
            " AND ca.ownerId = ?"
        } else {
            ""
        }
    );
    let mut query = sqlx::query_as::<_, (Option<i64>, Option<i64>)>(&sql).bind(today_start);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    let (today_success, today_failed) = query.fetch_one(db).await?;

    // 总签到次数和成功率（请求时间范围内）
    let sql = format!(
        "SELECT COUNT(*) as total, SUM(CASE WHEN status IN ('success', 'already_checked') THEN 1 ELSE 0 END) as success
         FROM CheckinRun cr
         JOIN CheckinAccount ca ON cr.accountId = ca.id
         WHERE cr.createdAt >= ? AND cr.createdAt <= ?{}",
        if owner_id.is_some() {
            " AND ca.ownerId = ?"
        } else {
            ""
        }
    );
    let mut query = sqlx::query_as::<_, (i64, Option<i64>)>(&sql)
        .bind(start)
        .bind(end);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    let (total_runs, success_count) = query.fetch_one(db).await?;
    let success_rate = if total_runs > 0 {
        success_count.unwrap_or(0) as f64 / total_runs as f64 * 100.0
    } else {
        0.0
    };

    // 总余额（美元，500000 quota = $1）
    let sql = format!(
        "SELECT SUM(COALESCE(lastBalance, 0)) FROM CheckinAccount WHERE enabled = 1{}",
        owner_filter
    );
    let mut query = sqlx::query_as::<_, (Option<f64>,)>(&sql);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    let (total_quota,) = query.fetch_one(db).await?;
    let total_balance = total_quota.unwrap_or(0.0) / 500000.0;

    Ok(Overview {
        total_accounts,
        enabled_accounts,
        today_success: today_success.unwrap_or(0),
        today_failed: today_failed.unwrap_or(0),
        total_runs,
        success_rate,
        total_balance,
    })
}

async fn calculate_daily_trend(
    db: &sqlx::SqlitePool,
    owner_id: Option<&str>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<DailyStats>> {
    let sql = format!(
        "SELECT
            DATE(cr.createdAt, 'localtime') as date,
            SUM(CASE WHEN cr.status = 'success' THEN 1 ELSE 0 END) as success,
            SUM(CASE WHEN cr.status = 'failed' THEN 1 ELSE 0 END) as failed,
            SUM(CASE WHEN cr.status = 'already_checked' THEN 1 ELSE 0 END) as already_checked,
            COUNT(*) as total
         FROM CheckinRun cr
         JOIN CheckinAccount ca ON cr.accountId = ca.id
         WHERE cr.createdAt >= ? AND cr.createdAt <= ?{}
         GROUP BY DATE(cr.createdAt, 'localtime')
         ORDER BY date ASC",
        if owner_id.is_some() {
            " AND ca.ownerId = ?"
        } else {
            ""
        }
    );

    let mut query = sqlx::query_as::<_, (String, i64, i64, i64, i64)>(&sql)
        .bind(start)
        .bind(end);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }

    let rows = query.fetch_all(db).await?;

    Ok(rows
        .into_iter()
        .map(|(date, success, failed, already_checked, total)| {
            let success_rate = if total > 0 {
                (success + already_checked) as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            DailyStats {
                date,
                success,
                failed,
                already_checked,
                total,
                success_rate,
            }
        })
        .collect())
}

async fn calculate_site_stats(
    db: &sqlx::SqlitePool,
    owner_id: Option<&str>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<SiteStats>> {
    let sql = format!(
        "SELECT
            ca.siteType,
            COUNT(DISTINCT ca.id) as accountCount,
            COUNT(cr.id) as totalRuns,
            SUM(CASE WHEN cr.status IN ('success', 'already_checked') THEN 1 ELSE 0 END) as success,
            SUM(CASE WHEN cr.status = 'failed' THEN 1 ELSE 0 END) as failed,
            AVG(COALESCE(cr.durationMs, 0)) as avgDuration
         FROM CheckinAccount ca
         LEFT JOIN CheckinRun cr ON ca.id = cr.accountId
            AND cr.createdAt >= ? AND cr.createdAt <= ?
         WHERE 1=1{}
         GROUP BY ca.siteType
         ORDER BY accountCount DESC",
        if owner_id.is_some() {
            " AND ca.ownerId = ?"
        } else {
            ""
        }
    );

    let mut query =
        sqlx::query_as::<_, (String, i64, i64, Option<i64>, Option<i64>, Option<f64>)>(&sql)
            .bind(start)
            .bind(end);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }

    let rows = query.fetch_all(db).await?;

    Ok(rows
        .into_iter()
        .map(
            |(site_type, account_count, total_runs, success, failed, avg_duration)| {
                let success = success.unwrap_or(0);
                let failed = failed.unwrap_or(0);
                let success_rate = if total_runs > 0 {
                    success as f64 / total_runs as f64 * 100.0
                } else {
                    0.0
                };
                SiteStats {
                    site_type,
                    account_count,
                    total_runs,
                    success,
                    failed,
                    success_rate,
                    avg_duration: avg_duration.unwrap_or(0.0),
                }
            },
        )
        .collect())
}

async fn calculate_balance_trend(
    db: &sqlx::SqlitePool,
    owner_id: Option<&str>,
) -> Result<Vec<BalanceTrend>> {
    // 由于没有历史余额记录，这里返回当前余额快照
    // 未来可以考虑在每次签到后记录余额变化到单独的表
    let owner_filter = if owner_id.is_some() {
        " AND ownerId = ?"
    } else {
        ""
    };
    let sql = format!(
        "SELECT SUM(COALESCE(lastBalance, 0)) FROM CheckinAccount WHERE enabled = 1{}",
        owner_filter
    );
    let mut query = sqlx::query_as::<_, (Option<f64>,)>(&sql);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }
    let (total_quota,) = query.fetch_one(db).await?;
    let balance = total_quota.unwrap_or(0.0) / 500000.0;

    // 返回今天的余额快照
    let today = Local::now().format("%Y-%m-%d").to_string();
    Ok(vec![BalanceTrend {
        date: today,
        balance,
    }])
}

async fn calculate_recent_failures(
    db: &sqlx::SqlitePool,
    owner_id: Option<&str>,
) -> Result<Vec<RecentFailure>> {
    let sql = format!(
        "SELECT
            cr.id,
            cr.accountId,
            ca.name,
            ca.siteType,
            u.username,
            cr.message,
            cr.createdAt
         FROM CheckinRun cr
         JOIN CheckinAccount ca ON cr.accountId = ca.id
         LEFT JOIN AppUser u ON ca.ownerId = u.id
         WHERE cr.status = 'failed'{}
         ORDER BY cr.createdAt DESC
         LIMIT 10",
        if owner_id.is_some() {
            " AND ca.ownerId = ?"
        } else {
            ""
        }
    );

    let mut query = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            DateTime<Utc>,
        ),
    >(&sql);
    if let Some(owner_id) = owner_id {
        query = query.bind(owner_id);
    }

    let rows = query.fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(
            |(run_id, account_id, account_name, site_type, owner_name, message, created_at)| {
                RecentFailure {
                    run_id,
                    account_id,
                    account_name,
                    site_type,
                    owner_name,
                    message,
                    created_at,
                }
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{calculate_recent_failures, resolve_owner_filter};
    use crate::{error::AppError, models::AppUser};
    use chrono::Utc;
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    fn user(id: &str, role: &str) -> AppUser {
        let now = Utc::now();
        AppUser {
            id: id.to_string(),
            username: format!("{id}-name"),
            password_hash: String::new(),
            role: role.to_string(),
            enabled: true,
            note: None,
            created_at: now,
            updated_at: now,
        }
    }

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
                ownerId TEXT,
                authType TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                retryEnabled INTEGER NOT NULL,
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

        pool
    }

    async fn insert_user(pool: &SqlitePool, id: &str, username: &str) {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO AppUser (id, username, passwordHash, role, enabled, createdAt, updatedAt)
             VALUES (?, ?, 'hash', 'USER', 1, ?, ?)",
        )
        .bind(id)
        .bind(username)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("user should be inserted");
    }

    async fn insert_account(pool: &SqlitePool, id: &str, owner_id: &str, name: &str) {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinAccount (
                id, name, siteType, baseUrl, ownerId, authType,
                enabled, retryEnabled, createdAt, updatedAt
             ) VALUES (?, ?, 'new-api', 'https://example.com', ?, 'access_token', 1, 1, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(owner_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("account should be inserted");
    }

    async fn insert_run(
        pool: &SqlitePool,
        id: &str,
        account_id: &str,
        status: &str,
        message: &str,
        created_at: chrono::DateTime<Utc>,
    ) {
        sqlx::query(
            "INSERT INTO CheckinRun (
                id, accountId, status, message, durationMs, triggeredBy, createdAt
             ) VALUES (?, ?, ?, ?, 10, 'manual', ?)",
        )
        .bind(id)
        .bind(account_id)
        .bind(status)
        .bind(message)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("run should be inserted");
    }

    #[test]
    fn admin_statistics_filter_can_target_any_user_or_all_users() {
        let admin = user("admin-id", "ADMIN");

        assert_eq!(resolve_owner_filter(&admin, true, None).unwrap(), None);
        assert_eq!(
            resolve_owner_filter(&admin, true, Some("target-id")).unwrap(),
            Some("target-id".to_string())
        );
        assert_eq!(
            resolve_owner_filter(&admin, true, Some("  ")).unwrap(),
            None
        );
    }

    #[test]
    fn regular_user_statistics_filter_is_limited_to_self() {
        let regular = user("user-id", "USER");

        assert_eq!(
            resolve_owner_filter(&regular, false, None).unwrap(),
            Some("user-id".to_string())
        );
        assert_eq!(
            resolve_owner_filter(&regular, false, Some("user-id")).unwrap(),
            Some("user-id".to_string())
        );
        assert!(matches!(
            resolve_owner_filter(&regular, false, Some("other-id")),
            Err(AppError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn recent_failures_are_limited_to_owner_and_sorted_newest_first() {
        let pool = test_pool().await;
        insert_user(&pool, "user-a", "alice").await;
        insert_user(&pool, "user-b", "bob").await;
        insert_account(&pool, "account-a", "user-a", "Alice API").await;
        insert_account(&pool, "account-b", "user-b", "Bob API").await;

        let older = Utc::now() - chrono::Duration::hours(2);
        let newer = Utc::now();
        insert_run(
            &pool,
            "run-a-old",
            "account-a",
            "failed",
            "old failure",
            older,
        )
        .await;
        insert_run(
            &pool,
            "run-a-new",
            "account-a",
            "failed",
            "new failure",
            newer,
        )
        .await;
        insert_run(&pool, "run-a-ok", "account-a", "success", "success", newer).await;
        insert_run(
            &pool,
            "run-b-new",
            "account-b",
            "failed",
            "other user failure",
            newer,
        )
        .await;

        let failures = calculate_recent_failures(&pool, Some("user-a"))
            .await
            .expect("recent failures should load");

        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].run_id, "run-a-new");
        assert_eq!(failures[1].run_id, "run-a-old");
        assert_eq!(failures[0].account_name, "Alice API");
        assert_eq!(failures[0].owner_name.as_deref(), Some("alice"));
        assert!(failures
            .iter()
            .all(|failure| failure.account_id == "account-a"));
    }
}
