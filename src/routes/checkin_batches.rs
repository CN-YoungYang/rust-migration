use crate::{
    business_time, db,
    error::{sanitize_user_message, AppError, Result},
    models::{AppUser, CheckinBatch, CheckinBatchItem},
    services::checkin::batch::spawn_checkin_batch,
    services::checkin::runner::skip_reason_for_batch,
    AppState,
};
use axum::{
    extract::{Extension, Path, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CreateCheckinBatchRequest {
    #[serde(rename = "accountIds")]
    pub account_ids: Vec<String>,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckinBatchResponse {
    #[serde(rename = "batchId")]
    pub batch_id: String,
    pub status: String,
    pub total: i64,
    pub completed: i64,
    pub succeeded: i64,
    #[serde(rename = "alreadyChecked")]
    pub already_checked: i64,
    pub skipped: i64,
    pub failed: i64,
    #[serde(rename = "triggeredBy")]
    pub triggered_by: String,
    #[serde(rename = "createdAt")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "startedAt")]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "canResume")]
    pub can_resume: bool,
    pub items: Vec<CheckinBatchItem>,
}

fn response_from_parts(
    batch: CheckinBatch,
    items: Vec<CheckinBatchItem>,
    can_resume: bool,
) -> CheckinBatchResponse {
    CheckinBatchResponse {
        batch_id: batch.id,
        status: batch.status,
        total: batch.total,
        completed: batch.completed,
        succeeded: batch.succeeded,
        already_checked: batch.already_checked,
        skipped: batch.skipped,
        failed: batch.failed,
        triggered_by: batch.triggered_by,
        created_at: batch.created_at,
        started_at: batch.started_at,
        finished_at: batch.finished_at,
        can_resume,
        items,
    }
}

fn is_admin(user: &AppUser) -> bool {
    user.role == "ADMIN" || user.role == "SUPER_ADMIN"
}

fn dedupe_ids(ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect()
}

fn normalize_idempotency_key(
    payload_key: Option<String>,
    headers: &HeaderMap,
) -> Result<Option<String>> {
    let header_key = headers
        .get("idempotency-key")
        .map(|value| value.to_str().map(str::to_string))
        .transpose()
        .map_err(|_| AppError::Validation("Idempotency-Key 必须是有效文本".into()))?;
    let key = payload_key.or(header_key);
    let key = key.map(|value| value.trim().to_string());
    if key.as_deref().is_some_and(str::is_empty) {
        return Ok(None);
    }
    if key.as_deref().is_some_and(|value| value.len() > 200) {
        return Err(AppError::Validation("幂等标识不能超过 200 个字符".into()));
    }
    Ok(key)
}

fn conflict_message(conflicts: &[db::ActiveBatchConflict]) -> String {
    let mut seen = HashSet::new();
    let details: Vec<String> = conflicts
        .iter()
        .filter(|conflict| seen.insert(conflict.account_id.clone()))
        .map(|conflict| {
            let state = if conflict.batch_status == "pending" {
                "等待执行"
            } else {
                "执行中"
            };
            format!(
                "{}（批次 {}，{}，进度 {}/{}）",
                conflict.account_id,
                conflict.batch_id,
                state,
                conflict.batch_completed,
                conflict.batch_total
            )
        })
        .collect();
    format!(
        "以下账户已在执行中批次，未创建新批次：{}",
        details.join("、")
    )
}

async fn load_response(
    db: &sqlx::SqlitePool,
    batch_id: &str,
    owner_id: Option<&str>,
    can_resume: bool,
) -> Result<CheckinBatchResponse> {
    let batch = db::find_checkin_batch(db, batch_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut items = match owner_id {
        Some(owner_id) => {
            db::list_checkin_batch_items_for_account_owner(db, batch_id, owner_id).await?
        }
        None => db::list_checkin_batch_items(db, batch_id).await?,
    };
    if owner_id.is_some() && items.is_empty() {
        return Err(AppError::Forbidden);
    }
    let batch = if owner_id.is_some() {
        db::scope_checkin_batch_to_items(batch, &items)
    } else {
        batch
    };
    for item in &mut items {
        item.message = item
            .message
            .take()
            .map(|message| sanitize_user_message(&message));
    }
    Ok(response_from_parts(batch, items, can_resume))
}

async fn can_control_batch(
    db: &sqlx::SqlitePool,
    batch: &CheckinBatch,
    user: &AppUser,
) -> Result<bool> {
    if is_admin(user) {
        return Ok(true);
    }
    if batch.created_by != user.id {
        return Ok(false);
    }
    db::checkin_batch_items_all_owned_by(db, &batch.id, &user.id).await
}

async fn load_response_for_user(
    db: &sqlx::SqlitePool,
    batch_id: &str,
    user: &AppUser,
) -> Result<CheckinBatchResponse> {
    let batch = db::find_checkin_batch(db, batch_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let can_resume = can_control_batch(db, &batch, user).await?;
    let owner_id = (!is_admin(user)).then_some(user.id.as_str());
    load_response(db, batch_id, owner_id, can_resume).await
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    headers: HeaderMap,
    Json(payload): Json<CreateCheckinBatchRequest>,
) -> Result<Json<Value>> {
    let idempotency_key = normalize_idempotency_key(payload.idempotency_key, &headers)?;
    if let Some(key) = idempotency_key.as_deref() {
        if let Some(batch) = db::find_checkin_batch_by_idempotency(&state.db, &user.id, key).await?
        {
            if batch.status == "pending" {
                spawn_checkin_batch(state.db.clone(), batch.id.clone());
            }
            return Ok(crate::routes::data(
                load_response_for_user(&state.db, &batch.id, &user).await?,
            ));
        }
    }

    let account_ids = dedupe_ids(payload.account_ids);
    if account_ids.is_empty() {
        return Err(AppError::Validation("accountIds 不能为空".into()));
    }
    if account_ids.len() > 500 {
        return Err(AppError::Validation(format!(
            "accountIds 数量不能超过 500，收到 {} 个（已去重）",
            account_ids.len()
        )));
    }

    let settings = db::get_settings(&state.db).await?;
    let today_local = business_time::today();
    let account_map = db::find_accounts_by_ids(&state.db, &account_ids).await?;
    let today_counts = db::count_runs_today_for_accounts(&state.db, &account_ids).await?;
    let admin = is_admin(&user);
    let mut items = Vec::with_capacity(account_ids.len());
    let mut pending_ids = Vec::new();

    // 先完整校验权限，确保批次创建不会出现“部分账户已入队”的歧义。
    for account_id in &account_ids {
        let account = account_map.get(account_id).ok_or(AppError::NotFound)?;
        if !admin && account.owner_id.as_deref() != Some(user.id.as_str()) {
            return Err(AppError::Forbidden);
        }
    }

    for account_id in &account_ids {
        let account = account_map.get(account_id).ok_or(AppError::NotFound)?;
        let (status, message) =
            if let Some(reason) = skip_reason_for_batch(account, &settings, today_local) {
                (
                    reason.status().to_string(),
                    Some(reason.message().to_string()),
                )
            } else if today_counts.get(account_id).copied().unwrap_or(0)
                >= settings.max_attempts_per_day.max(1)
            {
                (
                    "skipped".to_string(),
                    Some(format!(
                        "已达到今日最大尝试次数（{}）",
                        settings.max_attempts_per_day
                    )),
                )
            } else {
                pending_ids.push(account_id.clone());
                ("pending".to_string(), None)
            };
        items.push(db::NewCheckinBatchItem {
            account_id: account_id.clone(),
            account_name: account.name.clone(),
            status,
            message,
        });
    }

    let conflicts = db::find_active_batch_conflicts(&state.db, &pending_ids).await?;
    if !conflicts.is_empty() {
        return Err(AppError::Conflict(conflict_message(&conflicts)));
    }

    let batch =
        match db::create_checkin_batch(&state.db, &user.id, idempotency_key.as_deref(), &items)
            .await
        {
            Ok(batch) => batch,
            Err(AppError::Database(error))
                if error.to_string().contains("CheckinBatchItem.accountId") =>
            {
                let conflicts = db::find_active_batch_conflicts(&state.db, &pending_ids).await?;
                if conflicts.is_empty() {
                    return Err(AppError::Conflict(
                        "部分账户刚刚被其他签到批次占用，请刷新后重试".into(),
                    ));
                }
                return Err(AppError::Conflict(conflict_message(&conflicts)));
            }
            Err(AppError::Database(error))
                if error
                    .to_string()
                    .contains("CheckinBatch.createdBy, CheckinBatch.idempotencyKey") =>
            {
                let key = idempotency_key
                    .as_deref()
                    .ok_or_else(|| AppError::Internal("幂等批次冲突但请求未携带幂等标识".into()))?;
                let batch = db::find_checkin_batch_by_idempotency(&state.db, &user.id, key)
                    .await?
                    .ok_or_else(|| AppError::Conflict("批次正在创建，请稍后查询".into()))?;
                return Ok(crate::routes::data(
                    load_response_for_user(&state.db, &batch.id, &user).await?,
                ));
            }
            Err(error) => return Err(error),
        };

    if batch.status == "pending" {
        spawn_checkin_batch(state.db.clone(), batch.id.clone());
    }
    Ok(crate::routes::data(
        load_response_for_user(&state.db, &batch.id, &user).await?,
    ))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Path(batch_id): Path<String>,
) -> Result<Response> {
    let batch = db::find_checkin_batch(&state.db, &batch_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let response = crate::routes::data(load_response_for_user(&state.db, &batch.id, &user).await?)
        .into_response();
    let mut response = response;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    Ok(response)
}

/// 手动重新唤起因服务重启或入队异常而停留在 pending 的批次。
/// 不自动续跑，只有用户明确点击恢复入口时才重新入队。
pub async fn resume(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Path(batch_id): Path<String>,
) -> Result<Response> {
    let batch = db::find_checkin_batch(&state.db, &batch_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if !can_control_batch(&state.db, &batch, &user).await? {
        return Err(AppError::Forbidden);
    }
    let recovered = db::recover_stale_checkin_batch(&state.db, &batch_id).await?;
    let should_spawn = batch.status == "pending"
        || recovered
        || (batch.status == "running"
            && db::has_resumable_checkin_batch_items(&state.db, &batch_id).await?);
    if should_spawn {
        spawn_checkin_batch(state.db.clone(), batch.id.clone());
    }
    let response =
        crate::routes::data(load_response(&state.db, &batch_id, None, true).await?).into_response();
    let mut response = response;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    Ok(response)
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Response> {
    let active_only = params
        .get("active")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let limit = params
        .get("limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 100);
    let batches = if is_admin(&user) {
        db::list_checkin_batches(&state.db, None, active_only, limit).await?
    } else {
        db::list_checkin_batches_for_account_owner(&state.db, &user.id, active_only, limit).await?
    };
    let mut values = Vec::with_capacity(batches.len());
    for batch in batches {
        values.push(load_response_for_user(&state.db, &batch.id, &user).await?);
    }
    let response = crate::routes::data(values).into_response();
    let mut response = response;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::{dedupe_ids, normalize_idempotency_key};
    use axum::http::HeaderMap;

    #[test]
    fn batch_ids_are_trimmed_and_deduplicated() {
        assert_eq!(
            dedupe_ids(vec![" a ".into(), "b".into(), "a".into(), "".into()]),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn idempotency_key_accepts_body_or_header_and_rejects_empty_after_trim() {
        let mut headers = HeaderMap::new();
        headers.insert("Idempotency-Key", "header-key".parse().unwrap());
        assert_eq!(
            normalize_idempotency_key(None, &headers)
                .unwrap()
                .as_deref(),
            Some("header-key")
        );
        assert_eq!(
            normalize_idempotency_key(Some("  ".into()), &headers).unwrap(),
            None
        );
    }
}
