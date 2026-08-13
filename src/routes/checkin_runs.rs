use crate::{
    db,
    error::{AppError, Result},
    models::AppUser,
    services::checkin::runner::{execute_checkin, skip_reason_for_batch},
    AppState,
};
use axum::{
    extract::{Extension, Path, State},
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

/// 批量请求去重（Low2）：同一 id 重复提交只执行一次，保持首次出现顺序。
fn dedupe_ids(ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

/// 批量签到账户去重（保留旧函数名以兼容既有调用与测试）。
fn dedupe_account_ids(ids: Vec<String>) -> Vec<String> {
    dedupe_ids(ids)
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
        .clamp(1, 500);
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

    // 日期按本地日历日解释（与统计接口一致）：startDate 取当日 00:00，
    // endDate 取当日 23:59:59.999，避免结束日当天记录因零点边界被整日排除。
    // 无法按 %Y-%m-%d 解析时回退为原始字符串（兼容旧的 ISO 时间戳入参）。
    let (start_date, end_date) = resolve_date_bounds(filter_start_date, filter_end_date)?;

    let runs = db::list_runs_filtered(
        &state.db,
        &db::RunFilter {
            owner_id: owner_id.map(|s| s.to_string()),
            account_id: filter_account_id.map(|s| s.to_string()),
            status: filter_status.map(|s| s.to_string()),
            triggered_by: filter_triggered_by.map(|s| s.to_string()),
            start_date,
            end_date,
            limit,
            offset,
        },
    )
    .await?;
    Ok(crate::routes::data(runs))
}

/// 把日期入参转换为比较用时间戳：
/// - `YYYY-MM-DD`（日历日）：按服务器本地日界解释（与统计/调度口径一致）。
/// - 含 `T` 的完整时间戳（如浏览器本地日界转成的 RFC3339）：原样透传，作为绝对时刻比较。
///
/// 无法按 `%Y-%m-%d` 解析且不含 `T` 时回退为原始字符串（兼容旧入参）。
fn resolve_date_bounds(
    start: Option<&str>,
    end: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    use chrono::NaiveDate;

    let start_date = match start.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) if !s.contains('T') => match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(d) => Some(crate::routes::statistics::local_day_start(d)?.to_rfc3339()),
            Err(_) => Some(s.to_string()),
        },
        Some(s) => Some(s.to_string()),
        None => None,
    };
    let end_date = match end.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) if !s.contains('T') => match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(d) => Some(crate::routes::statistics::local_day_end(d)?.to_rfc3339()),
            Err(_) => Some(s.to_string()),
        },
        Some(s) => Some(s.to_string()),
        None => None,
    };
    Ok((start_date, end_date))
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
    // Low2 + M15：去重并限制批量数量。同一账户重复提交会导致二次签到（浪费且可能
    // 产生假失败记录），且 `count_runs_today_for_accounts` / `find_accounts_by_ids` 的
    // IN 子句占位符受 SQLite 绑定上限约束，故整体封顶 500。
    let account_ids = dedupe_account_ids(payload.account_ids);
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
    let today_local = chrono::Local::now().date_naive();
    let is_admin = user.role == "ADMIN" || user.role == "SUPER_ADMIN";

    // 批量查询今日各账户签到次数，避免逐账户 COUNT
    let mut today_counts = db::count_runs_today_for_accounts(&state.db, &account_ids)
        .await
        .unwrap_or_default();

    let mut items: Vec<BatchResultItem> = Vec::new();
    let mut to_execute: Vec<(String, String)> = Vec::new(); // (account_id, account_name)

    // 批量查询账户，替代逐个 find_account_by_id（N+1 → 1 次查询）
    let account_map = db::find_accounts_by_ids(&state.db, &account_ids).await?;

    // 阶段一：校验 + 跳过判断（串行）
    // 权限：任一账户无归属权即整体拒绝，避免部分执行带来的混淆。
    for account_id in &account_ids {
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
                // 只对真实尝试（success/failed）累加内存计数，与 DB 计数口径一致；
                // already_checked/skipped 不计入每日上限（M6）。
                if db::is_real_attempt(&run.status) {
                    *today_counts.entry(account_id.clone()).or_insert(0) += 1;
                }
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
                // Low8：脱敏后再回显到批量结果，避免 sqlx/DB 内部细节进入 UI
                message: Some(e.user_message()),
            }),
        }
    }

    // 按请求顺序排序结果，便于前端对照
    let order: std::collections::HashMap<&str, usize> = account_ids
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

/// ?????????????????????????????????????
pub async fn delete_run(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let run = db::find_run_by_id(&state.db, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    let account = db::find_account_by_id(&state.db, &run.account_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let is_admin = user.role == "ADMIN" || user.role == "SUPER_ADMIN";
    if !is_admin && account.owner_id.as_ref() != Some(&user.id) {
        return Err(AppError::Forbidden);
    }

    let deleted = db::delete_run(&state.db, &id).await?;
    if !deleted {
        return Err(AppError::NotFound);
    }

    Ok(crate::routes::data(json!({ "success": true, "id": id })))
}

#[derive(Debug, Deserialize)]
pub struct BatchDeleteRunsRequest {
    #[serde(rename = "runIds")]
    pub run_ids: Vec<String>,
}

/// 批量删除签到记录：一次事务内删除所选记录并逐账户重算状态。
///
/// 全或无：任一 id 不存在（含并发清理导致的缺失）或归属权越权即整体拒绝，
/// 不删除任何记录——与批量签到 `execute_batch` 的“任一账户无归属权即整体拒绝”
/// 哲学一致，也避免前端面对“删了一半”的中间状态。
pub async fn batch_delete_runs(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Json(payload): Json<BatchDeleteRunsRequest>,
) -> Result<Json<Value>> {
    // 去重并限制批量数量（与 execute_batch 一致）：IN 子句占位符受 SQLite
    // 绑定上限约束，且避免同一记录重复提交造成重复删除语义混淆。
    let run_ids = dedupe_ids(payload.run_ids);
    check_batch_delete_size(&run_ids)?;

    let is_admin = user.role == "ADMIN" || user.role == "SUPER_ADMIN";

    // 前置解析：一次查出目标 run 行（存在性校验交给 validate_batch_delete_targets）。
    let runs = db::find_runs_by_ids(&state.db, &run_ids).await?;
    let account_map = if is_admin {
        std::collections::HashMap::new()
    } else {
        let account_ids: Vec<String> = runs.iter().map(|r| r.account_id.clone()).collect();
        db::find_accounts_by_ids(&state.db, &account_ids).await?
    };
    validate_batch_delete_targets(&run_ids, &runs, is_admin, &user.id, &account_map)?;

    // strict=true：删除事务内重新核验全部目标行存在，关闭“预检与删除之间”的
    // TOCTOU 窗口（10 分钟清理并发删行时仍满足全或无契约）。
    let deleted = db::delete_runs_by_ids(&state.db, &run_ids, true).await?;
    tracing::info!(
        operator_id = %user.id,
        run_count = deleted,
        "批量删除签到记录完成"
    );

    Ok(crate::routes::data(json!({ "deletedCount": deleted })))
}

/// 批量删除请求的数量校验（全或无）：空列表与超过上限（500）整体拒绝。
/// 必须在查询前调用：超量的 IN 子句会突破 SQLite 占位符上限，应先给出干净的 400。
fn check_batch_delete_size(run_ids: &[String]) -> Result<()> {
    if run_ids.is_empty() {
        return Err(AppError::Validation("runIds 不能为空".into()));
    }
    if run_ids.len() > 500 {
        return Err(AppError::Validation(format!(
            "runIds 数量不能超过 500，收到 {} 个（已去重）",
            run_ids.len()
        )));
    }
    Ok(())
}

/// 批量删除目标记录的存在性与归属权校验（全或无）：
/// 查得条数 != 请求条数（含并发清理导致的缺失）即整体拒绝；非管理员任一记录
/// 无归属权也整体拒绝（与批量签到一致）。
fn validate_batch_delete_targets(
    run_ids: &[String],
    runs: &[crate::models::CheckinRun],
    is_admin: bool,
    user_id: &str,
    account_map: &std::collections::HashMap<String, crate::models::CheckinAccount>,
) -> Result<()> {
    if runs.len() != run_ids.len() {
        return Err(AppError::Validation(format!(
            "部分记录不存在或已被删除（请求 {} 条，仅找到 {} 条），未删除任何记录",
            run_ids.len(),
            runs.len()
        )));
    }
    if !is_admin {
        ensure_runs_owned_by_user(user_id, runs, account_map)?;
    }
    Ok(())
}

/// 校验批量删除目标记录的归属权：非管理员必须全部属于本人。
/// 任一无权（记录对应的账户不存在 / 归属他人）即返回错误，调用方据此整体拒绝。
fn ensure_runs_owned_by_user(
    user_id: &str,
    runs: &[crate::models::CheckinRun],
    account_map: &std::collections::HashMap<String, crate::models::CheckinAccount>,
) -> Result<()> {
    for run in runs {
        let account = account_map.get(&run.account_id).ok_or(AppError::NotFound)?;
        if account.owner_id.as_deref() != Some(user_id) {
            return Err(AppError::Forbidden);
        }
    }
    Ok(())
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
    use super::{
        check_batch_delete_size, dedupe_account_ids, ensure_runs_owned_by_user,
        resolve_cleanup_owner_scope, validate_batch_delete_targets,
    };
    use crate::error::AppError;
    use crate::models::{CheckinAccount, CheckinRun};
    use chrono::Utc;

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

    #[test]
    fn batch_ids_are_deduplicated_preserving_first_order() {
        assert_eq!(
            dedupe_account_ids(vec!["a".into(), "b".into(), "a".into(), "c".into()]),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert!(dedupe_account_ids(vec![]).is_empty());
        assert_eq!(
            dedupe_account_ids(vec!["x".into(), "x".into()]),
            vec!["x".to_string()]
        );
    }

    fn account(id: &str, owner: Option<&str>) -> CheckinAccount {
        let now = Utc::now();
        CheckinAccount {
            id: id.to_string(),
            name: format!("acct-{id}"),
            site_type: "new-api".into(),
            base_url: "http://example.com".into(),
            user_id: None,
            owner_id: owner.map(str::to_string),
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
            last_run_at: None,
            note: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn run(id: &str, account_id: &str) -> CheckinRun {
        CheckinRun {
            id: id.to_string(),
            account_id: account_id.to_string(),
            status: "success".into(),
            message: None,
            duration_ms: None,
            triggered_by: "manual".into(),
            raw_response: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn batch_delete_ownership_is_all_or_nothing() {
        let accounts = std::collections::HashMap::from([
            ("acc-1".to_string(), account("acc-1", Some("user-1"))),
            ("acc-2".to_string(), account("acc-2", Some("user-2"))),
        ]);

        // 全部属于本人 → 通过
        assert!(ensure_runs_owned_by_user(
            "user-1",
            &[run("r1", "acc-1"), run("r2", "acc-1")],
            &accounts
        )
        .is_ok());

        // 任一无权 → Forbidden（整体拒绝，与批量签到一致）
        assert!(matches!(
            ensure_runs_owned_by_user(
                "user-1",
                &[run("r1", "acc-1"), run("r3", "acc-2")],
                &accounts,
            ),
            Err(AppError::Forbidden)
        ));

        // 记录对应的账户不存在 → NotFound
        assert!(matches!(
            ensure_runs_owned_by_user("user-1", &[run("r9", "acc-missing")], &accounts),
            Err(AppError::NotFound)
        ));
    }

    #[test]
    fn batch_delete_size_rejects_empty_and_over_cap() {
        // 空 → Validation：缺 id 查不到时不能把空请求误当成功（全或无的边界）
        assert!(matches!(
            check_batch_delete_size(&[]),
            Err(AppError::Validation(_))
        ));

        // 恰好 500（去重后）→ Ok，占位符上限内
        let at_cap: Vec<String> = (0..500).map(|i| format!("id-{i}")).collect();
        assert!(check_batch_delete_size(&at_cap).is_ok());

        // 501 → Validation，必须在查询前拦截（超量 IN 子句会突破 SQLite 占位符上限）
        let over_cap: Vec<String> = (0..501).map(|i| format!("id-{i}")).collect();
        assert!(matches!(
            check_batch_delete_size(&over_cap),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn batch_delete_targets_reject_missing_ids_all_or_nothing() {
        let accounts = std::collections::HashMap::from([(
            "acc-1".to_string(),
            account("acc-1", Some("user-1")),
        )]);

        // 缺 id（请求 2 条、只找到 1 条）→ Validation 整体拒绝，管理员同样拒绝
        assert!(matches!(
            validate_batch_delete_targets(
                &["r1".into(), "r2".into()],
                &[run("r1", "acc-1")],
                false,
                "user-1",
                &accounts,
            ),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_batch_delete_targets(
                &["r1".into(), "r2".into()],
                &[run("r1", "acc-1")],
                true,
                "admin",
                &accounts,
            ),
            Err(AppError::Validation(_))
        ));

        // 全部找到 + 归属通过 → Ok
        assert!(validate_batch_delete_targets(
            &["r1".into()],
            &[run("r1", "acc-1")],
            false,
            "user-1",
            &accounts,
        )
        .is_ok());

        // 管理员跳过归属校验 → Ok
        assert!(validate_batch_delete_targets(
            &["r1".into()],
            &[run("r1", "acc-1")],
            true,
            "admin",
            &accounts,
        )
        .is_ok());
    }
}
