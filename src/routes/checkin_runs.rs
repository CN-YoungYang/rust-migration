use crate::{
    db,
    error::{AppError, Result},
    models::AppUser,
    services::checkin::runner::{execute_checkin, skip_reason_for_batch},
    AppState,
};
use axum::{
    extract::{Extension, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ExecuteCheckinRequest {
    #[serde(rename = "accountId")]
    pub account_id: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchCheckinRequest {
    #[serde(rename = "accountIds")]
    pub account_ids: Vec<String>,
}

/// 批量签到结果中的单项
#[derive(Debug, Serialize)]
pub struct BatchResultItem {
    #[serde(rename = "accountId")]
    account_id: String,
    #[serde(rename = "accountName")]
    account_name: String,
    status: String,
    message: Option<String>,
}

/// 批量签到响应（含汇总统计）
#[derive(Debug, Serialize)]
pub struct BatchCheckinResponse {
    #[serde(rename = "items")]
    items: Vec<BatchResultItem>,
    total: usize,
    succeeded: usize,
    skipped: usize,
    failed: usize,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>> {
    let filter_user_id = params.get("userId");
    let filter_status = params.get("status").map(|s| s.as_str());
    let filter_triggered_by = params.get("triggeredBy").map(|s| s.as_str());
    let filter_start_date = params.get("startDate").map(|s| s.as_str());
    let filter_end_date = params.get("endDate").map(|s| s.as_str());
    let filter_account_id = params.get("accountId").map(|s| s.as_str());
    let limit: i32 = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
        .min(500);
    let offset: i32 = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
        .max(0);

    let owner_id = if user.role == "ADMIN" || user.role == "SUPER_ADMIN" {
        filter_user_id.map(|s| s.as_str())
    } else {
        Some(user.id.as_str())
    };

    let runs = db::list_runs_filtered(
        &state.db,
        &db::RunFilter {
            owner_id: owner_id.map(|s| s.to_string()),
            account_id: filter_account_id.map(|s| s.to_string()),
            status: filter_status.map(|s| s.to_string()),
            triggered_by: filter_triggered_by.map(|s| s.to_string()),
            start_date: filter_start_date.map(|s| s.to_string()),
            end_date: filter_end_date.map(|s| s.to_string()),
            limit,
            offset,
        },
    )
    .await?;
    Ok(crate::routes::data(runs))
}

pub async fn execute(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<ExecuteCheckinRequest>,
) -> Result<Json<Value>> {
    let account = db::find_account_by_id(&state.db, &payload.account_id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    if user.role != "ADMIN"
        && user.role != "SUPER_ADMIN"
        && account.owner_id.as_ref() != Some(&user.id)
    {
        return Err(crate::error::AppError::Forbidden);
    }

    // 手动签到不受每日上限限制，前端会在达到上限时弹窗确认
    let run = execute_checkin(&state.db, &payload.account_id, "manual", None).await?;
    Ok(crate::routes::data(run))
}

pub async fn execute_batch(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Json(payload): Json<BatchCheckinRequest>,
) -> Result<Json<Value>> {
    if payload.account_ids.is_empty() {
        return Err(AppError::Validation("accountIds 不能为空".into()));
    }

    let settings = db::get_settings(&state.db).await?;
    let today_local = chrono::Local::now().date_naive();
    let is_admin = user.role == "ADMIN" || user.role == "SUPER_ADMIN";

    // 批量查询今日各账户签到次数，避免逐账户 COUNT
    let mut today_counts = db::count_runs_today_for_accounts(&state.db, &payload.account_ids)
        .await
        .unwrap_or_default();

    let mut items: Vec<BatchResultItem> = Vec::new();
    let mut to_execute: Vec<(String, String)> = Vec::new(); // (account_id, account_name)

    // 批量查询账户，替代逐个 find_account_by_id（N+1 → 1 次查询）
    let account_map = db::find_accounts_by_ids(&state.db, &payload.account_ids).await?;

    // 阶段一：校验 + 跳过判断（串行）
    // 权限：任一账户无归属权即整体拒绝，避免部分执行带来的混淆。
    for account_id in &payload.account_ids {
        let account = account_map
            .get(account_id.as_str())
            .ok_or(AppError::NotFound)?;

        // 归属权校验（与单次签到一致）
        if !is_admin && account.owner_id.as_ref() != Some(&user.id) {
            return Err(AppError::Forbidden);
        }

        let account_name = account.name.clone();

        // 跳过今日已签/已禁用/不允许重试
        if let Some(reason) = skip_reason_for_batch(account, &settings, today_local) {
            items.push(BatchResultItem {
                account_id: account_id.clone(),
                account_name: account_name.clone(),
                status: "skipped".to_string(),
                message: Some(reason.to_string()),
            });
            continue;
        }

        // 每日次数上限（使用批量查询结果）
        let today_runs = today_counts.get(account_id.as_str()).copied().unwrap_or(0);
        if today_runs >= settings.max_attempts_per_day.max(1) {
            items.push(BatchResultItem {
                account_id: account_id.clone(),
                account_name: account_name.clone(),
                status: "skipped".to_string(),
                message: Some(format!(
                    "已达到今日最大尝试次数 ({})",
                    settings.max_attempts_per_day
                )),
            });
            continue;
        }

        to_execute.push((account_id.clone(), account_name));
    }

    // 阶段二：串行执行 + 随机间隔 + 打乱顺序
    // 防判定：同一站点多账户瞬时并发是最大的机器人指纹，
    // 改为逐个签到，相邻账户之间按管理员设置随机延迟，并打乱执行顺序。
    use rand::seq::SliceRandom;
    to_execute.shuffle(&mut rand::thread_rng());

    tracing::info!(
        "批量手动签到开始：共 {} 个账户，串行执行，随机延迟 {}~{}s",
        to_execute.len(),
        settings.batch_delay_min,
        settings.batch_delay_max
    );

    for (idx, (account_id, account_name)) in to_execute.into_iter().enumerate() {
        // 首个账户不延迟，避免无谓等待；其余账户签到前随机 sleep
        if idx > 0 {
            if let Some(secs) = crate::services::checkin::random_delay_secs(
                settings.batch_delay_min,
                settings.batch_delay_max,
            ) {
                tracing::debug!("批量签到：账户 {} 等待 {}s 后执行", account_id, secs);
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            }
        }

        // 传入 settings 供 execute_checkin 做 TOCTOU 重检查
        match execute_checkin(&state.db, &account_id, "manual_batch", Some(&settings)).await {
            Ok(run) => {
                // 更新内存计数器，避免后续账户因过期计数而超限
                *today_counts.entry(account_id.clone()).or_insert(0) += 1;
                items.push(BatchResultItem {
                    account_id,
                    account_name,
                    status: run.status,
                    message: run.message,
                })
            }
            Err(e) => items.push(BatchResultItem {
                account_id,
                account_name,
                status: "failed".to_string(),
                message: Some(e.to_string()),
            }),
        }
    }

    // 按请求顺序排序结果，便于前端对照
    let order: std::collections::HashMap<&str, usize> = payload
        .account_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    items.sort_by_key(|it| {
        order
            .get(it.account_id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });

    let succeeded = items
        .iter()
        .filter(|it| it.status == "success" || it.status == "already_checked")
        .count();
    let skipped = items.iter().filter(|it| it.status == "skipped").count();
    let failed = items.iter().filter(|it| it.status == "failed").count();

    Ok(crate::routes::data(BatchCheckinResponse {
        total: items.len(),
        succeeded,
        skipped,
        failed,
        items,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CleanupRunsRequest {
    #[serde(rename = "keepLatest")]
    pub keep_latest: Option<i64>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "resetState", default)]
    pub reset_state: bool,
}

fn resolve_cleanup_owner_scope(
    role: &str,
    current_user_id: &str,
    requested_user_id: Option<&str>,
) -> Result<Option<String>> {
    let requested_user_id = requested_user_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if role == "ADMIN" || role == "SUPER_ADMIN" {
        return Ok(requested_user_id.map(str::to_string));
    }
    if requested_user_id.is_some_and(|requested| requested != current_user_id) {
        return Err(AppError::Forbidden);
    }
    Ok(Some(current_user_id.to_string()))
}

pub async fn cleanup_runs(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<CleanupRunsRequest>,
) -> Result<Json<Value>> {
    let keep_latest_raw = payload.keep_latest.unwrap_or(100);
    if !(0..=10000).contains(&keep_latest_raw) {
        return Err(AppError::Validation(format!(
            "keepLatest 必须在 0~10000 之间（0 表示清除全部），收到 {}",
            keep_latest_raw
        )));
    }
    if payload.reset_state && keep_latest_raw != 0 {
        return Err(AppError::Validation(
            "resetState 仅可在 keepLatest = 0 时使用".into(),
        ));
    }
    let keep_latest = keep_latest_raw as usize;
    let owner_id = resolve_cleanup_owner_scope(&user.role, &user.id, payload.user_id.as_deref())?;
    let result = db::cleanup_checkin_data(
        &state.db,
        keep_latest,
        owner_id.as_deref(),
        payload.reset_state,
    )
    .await?;

    tracing::info!(
        operator_id = %user.id,
        target_user_id = owner_id.as_deref().unwrap_or("ALL"),
        keep_latest,
        reset_state = payload.reset_state,
        deleted_runs = result.deleted_runs,
        "签到记录清理完成"
    );

    Ok(crate::routes::data(json!({
        "deletedCount": result.deleted_runs,
        "keepLatest": keep_latest,
        "resetAccountCount": result.reset_accounts,
        "deletedFailureCounterCount": result.deleted_failure_counters,
        "userId": owner_id
    })))
}
#[cfg(test)]
mod tests {
    use super::resolve_cleanup_owner_scope;
    use crate::error::AppError;

    #[test]
    fn cleanup_scope_enforces_user_ownership_and_admin_targeting() {
        assert!(matches!(
            resolve_cleanup_owner_scope("USER", "user-1", Some("user-2")),
            Err(AppError::Forbidden)
        ));
        assert_eq!(
            resolve_cleanup_owner_scope("USER", "user-1", None).expect("own scope"),
            Some("user-1".to_string())
        );
        assert_eq!(
            resolve_cleanup_owner_scope("ADMIN", "admin-1", Some("user-2"))
                .expect("admin target scope"),
            Some("user-2".to_string())
        );
        assert_eq!(
            resolve_cleanup_owner_scope("SUPER_ADMIN", "root-1", None).expect("global admin scope"),
            None
        );
    }
}
