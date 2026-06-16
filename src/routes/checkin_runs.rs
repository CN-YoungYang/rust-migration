use axum::{
    extract::{State, Extension},
    Json,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::{
    AppState,
    models::{AppUser, CheckinRun},
    error::{Result, AppError},
    db,
    services::checkin::runner::{execute_checkin, skip_reason_for_batch},
};

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
) -> Result<Json<Vec<CheckinRun>>> {
    let runs = if user.role == "ADMIN" || user.role == "SUPER_ADMIN" {
        db::list_runs(&state.db, 100).await?
    } else {
        db::list_runs_by_user(&state.db, &user.id, 100).await?
    };
    Ok(Json(runs))
}

pub async fn execute(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<ExecuteCheckinRequest>,
) -> Result<Json<CheckinRun>> {
    let account = db::find_account_by_id(&state.db, &payload.account_id).await?.ok_or(crate::error::AppError::NotFound)?;
    if user.role != "ADMIN" && user.role != "SUPER_ADMIN" && account.owner_id.as_ref() != Some(&user.id) {
        return Err(crate::error::AppError::Forbidden);
    }

    let today_count = db::count_runs_by_account_today(&state.db, &payload.account_id).await?;
    let settings = db::get_settings(&state.db).await?;
    if today_count >= settings.max_attempts_per_day {
        return Err(crate::error::AppError::Validation(
            format!("已达到今日最大尝试次数 ({})", settings.max_attempts_per_day)
        ));
    }

    let run = execute_checkin(&state.db, &payload.account_id, "manual").await?;
    Ok(Json(run))
}

pub async fn execute_batch(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Json(payload): Json<BatchCheckinRequest>,
) -> Result<Json<BatchCheckinResponse>> {
    if payload.account_ids.is_empty() {
        return Err(AppError::Validation("accountIds 不能为空".into()));
    }

    let settings = db::get_settings(&state.db).await?;
    let today_local = chrono::Local::now().date_naive();
    let is_admin = user.role == "ADMIN" || user.role == "SUPER_ADMIN";

    // 收集账户用户名用于结果展示
    let users = db::list_users(&state.db).await?;
    let user_name_map: std::collections::HashMap<&str, &str> = users
        .iter()
        .map(|u| (u.id.as_str(), u.username.as_str()))
        .collect();

    let mut items: Vec<BatchResultItem> = Vec::new();
    let mut to_execute: Vec<(String, String)> = Vec::new(); // (account_id, account_name)

    // 阶段一：校验 + 跳过判断（串行）
    // 权限：任一账户无归属权即整体拒绝，避免部分执行带来的混淆。
    for account_id in &payload.account_ids {
        let account = db::find_account_by_id(&state.db, account_id)
            .await?
            .ok_or(AppError::NotFound)?;

        // 归属权校验（与单次签到一致）
        if !is_admin && account.owner_id.as_ref() != Some(&user.id) {
            return Err(AppError::Forbidden);
        }

        let account_name = account
            .owner_id
            .as_deref()
            .and_then(|oid| user_name_map.get(oid))
            .copied()
            .unwrap_or("")
            .to_string();

        // 跳过今日已签/已禁用/不允许重试
        if let Some(reason) = skip_reason_for_batch(&account, &settings, today_local) {
            items.push(BatchResultItem {
                account_id: account_id.clone(),
                account_name: account_name.clone(),
                status: "skipped".to_string(),
                message: Some(reason.to_string()),
            });
            continue;
        }

        // 每日次数上限
        let today_runs = db::count_runs_by_account_today(&state.db, account_id).await?;
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

        match execute_checkin(&state.db, &account_id, "manual_batch").await {
            Ok(run) => items.push(BatchResultItem {
                account_id,
                account_name,
                status: run.status,
                message: run.message,
            }),
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
    items.sort_by_key(|it| order.get(it.account_id.as_str()).copied().unwrap_or(usize::MAX));

    let succeeded = items.iter().filter(|it| it.status == "success" || it.status == "already_checked").count();
    let skipped = items.iter().filter(|it| it.status == "skipped").count();
    let failed = items.iter().filter(|it| it.status == "failed").count();

    Ok(Json(BatchCheckinResponse {
        total: items.len(),
        succeeded,
        skipped,
        failed,
        items,
    }))
}

pub async fn cleanup_runs(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<Value>> {
    let keep_latest = payload["keepLatest"].as_i64().unwrap_or(100) as usize;
    
    let deleted_count = if user.role == "ADMIN" || user.role == "SUPER_ADMIN" {
        db::cleanup_checkin_runs(&state.db, keep_latest).await?
    } else {
        db::cleanup_checkin_runs_by_user(&state.db, &user.id, keep_latest).await?
    };
    
    Ok(Json(json!({
        "deletedCount": deleted_count,
        "keepLatest": keep_latest
    })))
}
