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

    // 计算概览数据
    let overview =
        calculate_overview(&state.db, &user, is_admin, start_datetime, end_datetime).await?;

    // 计算每日趋势
    let daily_trend =
        calculate_daily_trend(&state.db, &user, is_admin, start_datetime, end_datetime).await?;

    // 计算站点统计
    let site_stats =
        calculate_site_stats(&state.db, &user, is_admin, start_datetime, end_datetime).await?;

    // 计算余额趋势（最近30天）
    let balance_trend = calculate_balance_trend(&state.db, &user, is_admin).await?;

    Ok(crate::routes::data(StatisticsResponse {
        overview,
        daily_trend,
        site_stats,
        balance_trend,
    }))
}

async fn calculate_overview(
    db: &sqlx::SqlitePool,
    user: &crate::models::AppUser,
    is_admin: bool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Overview> {
    let owner_filter = if is_admin { "" } else { " AND ownerId = ?" };

    // 总账户数和已启用账户数
    let sql = format!("SELECT COUNT(*) as total, SUM(CASE WHEN enabled = 1 THEN 1 ELSE 0 END) as enabled FROM CheckinAccount WHERE 1=1{}", owner_filter);
    let mut query = sqlx::query_as::<_, (i64, Option<i64>)>(&sql);
    if !is_admin {
        query = query.bind(&user.id);
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
        if is_admin { "" } else { " AND ca.ownerId = ?" }
    );
    let mut query = sqlx::query_as::<_, (Option<i64>, Option<i64>)>(&sql).bind(today_start);
    if !is_admin {
        query = query.bind(&user.id);
    }
    let (today_success, today_failed) = query.fetch_one(db).await?;

    // 总签到次数和成功率（请求时间范围内）
    let sql = format!(
        "SELECT COUNT(*) as total, SUM(CASE WHEN status IN ('success', 'already_checked') THEN 1 ELSE 0 END) as success
         FROM CheckinRun cr
         JOIN CheckinAccount ca ON cr.accountId = ca.id
         WHERE cr.createdAt >= ? AND cr.createdAt <= ?{}",
        if is_admin { "" } else { " AND ca.ownerId = ?" }
    );
    let mut query = sqlx::query_as::<_, (i64, Option<i64>)>(&sql)
        .bind(start)
        .bind(end);
    if !is_admin {
        query = query.bind(&user.id);
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
    if !is_admin {
        query = query.bind(&user.id);
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
    user: &crate::models::AppUser,
    is_admin: bool,
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
        if is_admin { "" } else { " AND ca.ownerId = ?" }
    );

    let mut query = sqlx::query_as::<_, (String, i64, i64, i64, i64)>(&sql)
        .bind(start)
        .bind(end);
    if !is_admin {
        query = query.bind(&user.id);
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
    user: &crate::models::AppUser,
    is_admin: bool,
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
        if is_admin { "" } else { " AND ca.ownerId = ?" }
    );

    let mut query =
        sqlx::query_as::<_, (String, i64, i64, Option<i64>, Option<i64>, Option<f64>)>(&sql)
            .bind(start)
            .bind(end);
    if !is_admin {
        query = query.bind(&user.id);
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
    user: &crate::models::AppUser,
    is_admin: bool,
) -> Result<Vec<BalanceTrend>> {
    // 由于没有历史余额记录，这里返回当前余额快照
    // 未来可以考虑在每次签到后记录余额变化到单独的表
    let owner_filter = if is_admin { "" } else { " AND ownerId = ?" };
    let sql = format!(
        "SELECT SUM(COALESCE(lastBalance, 0)) FROM CheckinAccount WHERE enabled = 1{}",
        owner_filter
    );
    let mut query = sqlx::query_as::<_, (Option<f64>,)>(&sql);
    if !is_admin {
        query = query.bind(&user.id);
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
