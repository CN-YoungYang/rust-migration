use crate::{
    business_time, db,
    error::{sanitize_user_message, AppError, Result},
    models::{AppUser, CheckinAccount, CheckinBatch, CheckinBatchItem},
    services::checkin::runner::skip_reason_for_batch,
    AppState,
};
use axum::{
    extract::{Extension, Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, FixedOffset, Utc};
use croner::Cron;
use serde::Serialize;
use serde_json::{json, Value};
use std::{collections::HashMap, str::FromStr, sync::Arc};

const PAGE_SIZE: i64 = 50;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchSummaryResponse {
    batch_id: String,
    created_by: String,
    triggered_by: String,
    status: String,
    total: i64,
    completed: i64,
    succeeded: i64,
    already_checked: i64,
    skipped: i64,
    failed: i64,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleSummaryResponse {
    enabled: bool,
    timezone: &'static str,
    schedule_cron: Vec<String>,
    next_run_at: Option<DateTime<Utc>>,
    last_result: Option<db::WorkbenchScheduledRun>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchResponse {
    business_date: String,
    timezone: &'static str,
    generated_at: DateTime<Utc>,
    summary: db::WorkbenchSummary,
    accounts: Vec<Value>,
    total_accounts: i64,
    limit: i64,
    offset: i64,
    active_batches: Vec<BatchSummaryResponse>,
    recent_batches: Vec<BatchSummaryResponse>,
    balance_warnings: Vec<Value>,
    recent_failures: Vec<db::WorkbenchFailure>,
    auto_schedule: ScheduleSummaryResponse,
}

fn is_admin(user: &AppUser) -> bool {
    user.role == "ADMIN" || user.role == "SUPER_ADMIN"
}

fn owner_filter(user: &AppUser, requested: Option<&str>) -> Result<Option<String>> {
    let requested = requested.map(str::trim).filter(|value| !value.is_empty());
    if is_admin(user) {
        return Ok(requested.map(ToOwned::to_owned));
    }
    if requested.is_some_and(|value| value != user.id) {
        return Err(AppError::Forbidden);
    }
    Ok(Some(user.id.clone()))
}

fn parse_bool(params: &HashMap<String, String>, key: &str) -> Option<bool> {
    params
        .get(key)
        .and_then(|value| match value.to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        })
}

fn parse_today_result(params: &HashMap<String, String>) -> Option<String> {
    let value = params.get("todayResult")?.trim();
    matches!(
        value,
        "not_checked" | "success" | "already_checked" | "failed" | "pending"
    )
    .then(|| value.to_string())
}

fn account_today_result(account: &CheckinAccount, today: chrono::NaiveDate) -> &'static str {
    if !account.enabled {
        return "disabled";
    }
    let Some(last_run_at) = account.last_run_at else {
        return "not_checked";
    };
    if business_time::date_in_business_timezone(last_run_at) != today {
        return "not_checked";
    }
    match account.last_status.as_deref() {
        Some("success") => "success",
        Some("already_checked") => "already_checked",
        Some("failed") => "failed",
        Some("pending") => "pending",
        _ => "not_checked",
    }
}

fn serialize_account(
    account: &CheckinAccount,
    owner_name: Option<&str>,
    warning: bool,
    today_runs: i32,
    today: chrono::NaiveDate,
    settings: &crate::models::CheckinSetting,
) -> Result<Value> {
    let today_result = account_today_result(account, today);
    let skip_reason = skip_reason_for_batch(account, settings, today)
        .map(|reason| reason.message().to_string())
        .or_else(|| {
            if today_runs >= settings.max_attempts_per_day.max(1) {
                Some(format!(
                    "已达到今日最大尝试次数（{}）",
                    settings.max_attempts_per_day.max(1)
                ))
            } else {
                None
            }
        });
    let selectable = account.enabled && skip_reason.is_none();

    let mut value = serde_json::to_value(account)?;
    value["lastMessage"] = account
        .last_message
        .as_deref()
        .map(sanitize_user_message)
        .map_or(Value::Null, Value::String);
    value["ownerName"] = owner_name.map_or(Value::Null, |name| json!(name));
    value["balanceWarning"] = json!(warning);
    value["todayRuns"] = json!(today_runs);
    value["todayResult"] = json!(today_result);
    value["selectable"] = json!(selectable);
    value["skipReason"] = skip_reason.map_or(Value::Null, Value::String);
    Ok(value)
}

fn batch_summary(
    batch: CheckinBatch,
    visible_items: Option<&[CheckinBatchItem]>,
) -> BatchSummaryResponse {
    let batch = if let Some(items) = visible_items {
        db::scope_checkin_batch_to_items(batch, items)
    } else {
        batch
    };
    BatchSummaryResponse {
        batch_id: batch.id,
        created_by: batch.created_by,
        triggered_by: batch.triggered_by,
        status: batch.status,
        total: batch.total,
        completed: batch.completed,
        succeeded: batch.succeeded,
        already_checked: batch.already_checked,
        skipped: batch.skipped,
        failed: batch.failed,
        created_at: batch.created_at,
        started_at: batch.started_at,
        finished_at: batch.finished_at,
    }
}

fn next_schedule_at(exprs: &[String], now: DateTime<FixedOffset>) -> Option<DateTime<Utc>> {
    exprs
        .iter()
        .filter_map(|expression| {
            Cron::from_str(expression)
                .ok()
                .and_then(|cron| cron.find_next_occurrence(&now, false).ok())
        })
        .min()
        .map(|value| value.to_utc())
}

async fn account_values(
    state: &AppState,
    accounts: &[CheckinAccount],
    owner_id: Option<&str>,
    today: chrono::NaiveDate,
    settings: &crate::models::CheckinSetting,
    owner_map: Option<&HashMap<String, String>>,
    fallback_owner: Option<&str>,
) -> Result<Vec<Value>> {
    let ids: Vec<String> = accounts.iter().map(|account| account.id.clone()).collect();
    let today_counts = db::count_runs_today_for_accounts(&state.db, &ids).await?;
    let warning_ids = db::list_balance_warning_account_ids(&state.db, owner_id, &ids).await?;
    accounts
        .iter()
        .map(|account| {
            let owner_name = owner_map
                .and_then(|map| account.owner_id.as_deref().and_then(|id| map.get(id)))
                .map(String::as_str)
                .or(fallback_owner);
            serialize_account(
                account,
                owner_name,
                warning_ids.contains(&account.id),
                today_counts.get(&account.id).copied().unwrap_or(0),
                today,
                settings,
            )
        })
        .collect::<Result<Vec<_>>>()
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response> {
    let owner_id = owner_filter(&user, params.get("userId").map(String::as_str))?;
    let today = business_time::today();
    let today_start = business_time::day_start_utc(today)?;
    let limit = params
        .get("limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(PAGE_SIZE)
        .clamp(1, 100);
    let offset = params
        .get("offset")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let filter = db::WorkbenchAccountFilter {
        owner_id: owner_id.clone(),
        site_type: params
            .get("siteType")
            .map(String::as_str)
            .filter(|value| matches!(*value, "new-api" | "anyrouter" | "x666"))
            .map(ToOwned::to_owned),
        enabled: parse_bool(&params, "enabled"),
        status: params
            .get("status")
            .map(String::as_str)
            .filter(|value| {
                matches!(
                    *value,
                    "never" | "disabled" | "success" | "already_checked" | "failed" | "pending"
                )
            })
            .map(ToOwned::to_owned),
        today_result: parse_today_result(&params),
        balance_warning: parse_bool(&params, "balanceWarning"),
        keyword: params
            .get("keyword")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        today_start,
        limit,
        offset,
    };

    let settings = db::get_settings(&state.db).await?;
    let summary = db::get_workbench_summary(&state.db, owner_id.as_deref(), today_start).await?;
    let total_accounts = db::count_workbench_accounts(&state.db, &filter).await?;
    let accounts = db::list_workbench_accounts(&state.db, &filter).await?;
    let owner_map = if is_admin(&user) {
        Some(db::list_user_id_name_map(&state.db).await?)
    } else {
        None
    };
    let accounts = account_values(
        &state,
        &accounts,
        owner_id.as_deref(),
        today,
        &settings,
        owner_map.as_ref(),
        (!is_admin(&user)).then_some(user.username.as_str()),
    )
    .await?;

    let warning_filter = db::WorkbenchAccountFilter {
        owner_id: owner_id.clone(),
        balance_warning: Some(true),
        today_start,
        limit: 8,
        offset: 0,
        ..Default::default()
    };
    let warning_accounts = db::list_workbench_accounts(&state.db, &warning_filter).await?;
    let balance_warnings = account_values(
        &state,
        &warning_accounts,
        owner_id.as_deref(),
        today,
        &settings,
        owner_map.as_ref(),
        (!is_admin(&user)).then_some(user.username.as_str()),
    )
    .await?;

    let batches = if is_admin(&user) {
        if let Some(owner_id) = owner_id.as_deref() {
            db::list_checkin_batches_for_account_owner(&state.db, owner_id, false, 30).await?
        } else {
            db::list_checkin_batches(&state.db, None, false, 30).await?
        }
    } else {
        db::list_checkin_batches(&state.db, Some(user.id.as_str()), false, 30).await?
    };
    let mut active_batches = Vec::new();
    let mut recent_batches = Vec::new();
    for batch in batches {
        let visible_items = if let Some(owner_id) = owner_id.as_deref() {
            let items =
                db::list_checkin_batch_items_for_account_owner(&state.db, &batch.id, owner_id)
                    .await?;
            if items.is_empty() {
                continue;
            }
            Some(items)
        } else {
            None
        };
        let summary = batch_summary(batch, visible_items.as_deref());
        if matches!(summary.status.as_str(), "pending" | "running") {
            if active_batches.len() < 8 {
                active_batches.push(summary);
            }
        } else if recent_batches.len() < 8 {
            recent_batches.push(summary);
        }
    }

    let mut last_result = db::find_latest_scheduled_run(&state.db, owner_id.as_deref()).await?;
    if let Some(result) = last_result.as_mut() {
        result.message = result
            .message
            .take()
            .map(|message| sanitize_user_message(&message));
    }
    let schedule = ScheduleSummaryResponse {
        enabled: settings.enabled,
        timezone: business_time::TIMEZONE_NAME,
        schedule_cron: settings.schedule_cron.clone(),
        next_run_at: settings
            .enabled
            .then(|| next_schedule_at(&settings.schedule_cron, business_time::now()))
            .flatten(),
        last_result,
    };
    let mut recent_failures =
        db::list_workbench_failures(&state.db, owner_id.as_deref(), 8).await?;
    for failure in &mut recent_failures {
        failure.message = failure
            .message
            .take()
            .map(|message| sanitize_user_message(&message));
    }

    let response = crate::routes::data(WorkbenchResponse {
        business_date: today.to_string(),
        timezone: business_time::TIMEZONE_NAME,
        generated_at: Utc::now(),
        summary,
        accounts,
        total_accounts,
        limit,
        offset,
        active_batches,
        recent_batches,
        balance_warnings,
        recent_failures,
        auto_schedule: schedule,
    })
    .into_response();
    let mut response = response;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::{account_today_result, next_schedule_at};
    use crate::models::CheckinAccount;
    use chrono::{FixedOffset, TimeZone, Timelike, Utc};

    fn account() -> CheckinAccount {
        let now = Utc::now();
        CheckinAccount {
            id: "a1".into(),
            name: "测试账户".into(),
            site_type: "new-api".into(),
            base_url: "https://example.com".into(),
            user_id: None,
            owner_id: Some("u1".into()),
            auth_type: "access_token".into(),
            access_token_enc: None,
            cookie_enc: None,
            custom_checkin_url: None,
            enabled: true,
            retry_enabled: true,
            last_balance: None,
            last_balance_at: None,
            last_status: None,
            last_message: None,
            last_run_at: Some(now),
            note: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn schedule_returns_the_earliest_next_expression() {
        let now = FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 9, 17, 10, 1, 0)
            .single()
            .unwrap();
        let next = next_schedule_at(&["0 11 * * *".into(), "30 10 * * *".into()], now).unwrap();
        assert_eq!(
            next.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
                .hour(),
            10
        );
        assert_eq!(
            next.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
                .minute(),
            30
        );
    }

    #[test]
    fn today_result_marks_disabled_accounts_separately() {
        let mut account = account();
        let today = crate::business_time::today();
        account.enabled = false;
        assert_eq!(account_today_result(&account, today), "disabled");
    }
}
