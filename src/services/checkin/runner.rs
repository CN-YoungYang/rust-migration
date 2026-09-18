use super::providers::{anyrouter, new_api, x666};
use super::BrowserProfile;
use crate::{
    business_time,
    crypto::decrypt,
    db,
    error::{sanitize_user_message, AppError, Result},
    models::{CheckinAccount, CheckinRun, CheckinSetting},
};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const QUOTA_PER_USD: f64 = 500_000.0;

// ── 防并发签到：同一账户同时只能有一个签到操作在执行 ──────────────────────
// 定时签到、手动单个签到、手动批量签到共用此锁，避免同一账户被重复签到。
fn in_flight_accounts() -> &'static Mutex<HashSet<String>> {
    static INSTANCE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(HashSet::new()))
}

/// RAII 守卫：获取成功时插入 account_id，Drop 时自动移除。
struct InFlightGuard {
    account_id: String,
}

impl InFlightGuard {
    /// 尝试获取指定账户的签到锁。返回 `None` 表示该账户正在签到中。
    fn try_acquire(account_id: &str) -> Option<Self> {
        let mut set = in_flight_accounts()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if set.contains(account_id) {
            return None;
        }
        set.insert(account_id.to_string());
        Some(InFlightGuard {
            account_id: account_id.to_string(),
        })
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        let mut set = in_flight_accounts()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set.remove(&self.account_id);
    }
}

/// 批量/定时签到前对单个账户的跳过判断（不涉及 DB 计数查询，便于复用）。
/// 返回 `Some(reason)` 表示应跳过该账户，`None` 表示需要继续执行。
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchSkipReason {
    AccountDisabled,
    AlreadyChecked,
    RetryDisabled,
}

impl BatchSkipReason {
    pub fn status(self) -> &'static str {
        match self {
            Self::AlreadyChecked => "already_checked",
            Self::AccountDisabled | Self::RetryDisabled => "skipped",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::AccountDisabled => "账户已禁用",
            Self::AlreadyChecked => "今日已签到",
            Self::RetryDisabled => "重试已关闭",
        }
    }
}

/// 与 `scheduler` 内联判断保持一致：
/// - 已禁用 -> 账户已禁用
/// - 今日已 success/already_checked -> 今日已签到
/// - 今日已尝试且（全局或账户）关闭重试 -> 重试已关闭
pub fn skip_reason_for_batch(
    account: &CheckinAccount,
    settings: &CheckinSetting,
    today_local: chrono::NaiveDate,
) -> Option<BatchSkipReason> {
    if !account.enabled {
        return Some(BatchSkipReason::AccountDisabled);
    }

    if let Some(last_run) = account.last_run_at {
        if business_time::date_in_business_timezone(last_run) == today_local {
            if let Some(status) = &account.last_status {
                if status == "success" || status == "already_checked" {
                    return Some(BatchSkipReason::AlreadyChecked);
                }
            }

            // 今日已尝试且未成功：仅当全局和账户都允许重试时才继续
            if !settings.retry_enabled || !account.retry_enabled {
                return Some(BatchSkipReason::RetryDisabled);
            }
        }
    }

    None
}

pub async fn execute_checkin(
    db: &SqlitePool,
    account_id: &str,
    triggered_by: &str,
    settings: Option<&CheckinSetting>,
) -> Result<CheckinRun> {
    let start = Instant::now();

    // 防并发：同一账户同时只能有一个签到操作（定时/手动/批量共用）
    let _guard = match InFlightGuard::try_acquire(account_id) {
        Some(g) => g,
        None => {
            // 抢占失败不是真实签到尝试：返回 skipped（不落库），
            // 避免假失败记录消耗每日尝试预算并污染统计（M6）。
            return Ok(skipped_run(
                account_id,
                "该账户正在签到中，请稍后再试",
                triggered_by,
                start,
                "skipped",
            ));
        }
    };

    let account = db::find_account_by_id(db, account_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if !account.enabled {
        return Ok(skipped_run(
            account_id,
            "账户已禁用",
            triggered_by,
            start,
            "skipped",
        ));
    }

    // TOCTOU 重检查：用刚从 DB 取到的最新账户状态再做一次 skip 判断，
    // 避免调用方的 skip_reason_for_batch 与实际执行之间账户状态已变化。
    if let Some(s) = settings {
        let today_local = business_time::today();
        if let Some(reason) = skip_reason_for_batch(&account, s, today_local) {
            return Ok(skipped_run(
                account_id,
                reason.message(),
                triggered_by,
                start,
                reason.status(),
            ));
        }

        // M5：在单飞锁内用最新 DB 计数复核每日上限，关闭“调度与手动批量同时通过
        // 上限检查后各自执行”的竞态。手动单签（settings=None）不受每日上限限制。
        let today_runs = db::count_runs_today(db, account_id).await?;
        if today_runs >= s.max_attempts_per_day.max(1) {
            let msg = format!("已达到今日最大尝试次数 ({})", s.max_attempts_per_day);
            return Ok(skipped_run(
                account_id,
                &msg,
                triggered_by,
                start,
                "skipped",
            ));
        }
    }

    // SSRF 执行期复核：base_url 可能在上次配置写入后被 DNS 重绑定到内网，
    // 发送前再次解析并拒绝私网地址（fail-closed）。自定义签到 URL 已限定与
    // base_url 同源，因此复核 base_url 的主机即覆盖全部出站地址。
    if let Err(e) =
        crate::security::validate_public_http_url_resolved(&account.base_url, "签到地址").await
    {
        let detail = e.to_string();
        let msg = sanitize_user_message(&e.user_message());
        tracing::warn!(account_id = %account_id, error = %detail, "签到前 SSRF 复核未通过");
        // 视为一次真实失败尝试：更新账户 lastStatus/lastRunAt、失败计数并触发通知，
        // 与 provider 报错路径一致。否则 lastRunAt 停在昨日，`skip_reason_for_batch`
        // 的 retry_disabled 分支永不触发——关闭重试的账户会在每轮调度中反复尝试（回归修复）。
        let duration_ms = start.elapsed().as_millis().min(i64::MAX as u128) as i64;
        let run = db::create_run_with_status_update(
            db,
            account_id,
            "failed",
            Some(&msg),
            Some(duration_ms),
            triggered_by,
            None,
        )
        .await?;
        handle_notifications(db, &account, "failed", &msg, account.last_balance).await;
        return Ok(run);
    }

    // 防判定：每次签到使用随机 UA，降低多账户同 IP + 同 UA 的关联指纹。
    let profile = super::random_browser_profile();

    let result = match account.site_type.as_str() {
        "new-api" => execute_new_api_checkin(&account, profile).await,
        "anyrouter" => execute_anyrouter_checkin(&account, profile).await,
        "x666" => execute_x666_checkin(&account, profile).await,
        _ => Err(AppError::Validation(format!(
            "不支持的站点类型: {}",
            account.site_type
        ))),
    };

    let duration_ms = start.elapsed().as_millis().min(i64::MAX as u128) as i64;

    match result {
        Ok((status, message, raw_response)) => {
            let message = sanitize_user_message(&message);
            // 签到成功或今日已签时刷新余额（参考 Next.js runner.ts）
            // 余额刷新失败了不影响签到结果，仅在消息中追加提示。
            // 余额刷新为网络请求，无法并入 DB 事务；但其写库与状态更新、记录创建
            // 通过 create_run_with_status_update_and_balance 在同一事务原子提交，
            // 避免崩溃时出现"余额已更新但无签到记录"的部分写入。
            let mut notification_balance = account.last_balance.map(|quota| quota / QUOTA_PER_USD);
            let (balance_to_store, final_message) = if status.as_str() == "success"
                || status.as_str() == "already_checked"
            {
                match fetch_account_balance(&account, profile).await {
                    Ok(quota) => {
                        notification_balance = Some(quota / QUOTA_PER_USD);
                        (Some(quota), message)
                    }
                    Err(e) => {
                        let detail = e.to_string();
                        let msg = sanitize_user_message(&e.user_message());
                        tracing::warn!(account_id = %account_id, error = %detail, "签到后余额刷新失败");
                        (None, format!("{}；余额刷新失败：{}", message, msg))
                    }
                }
            } else {
                (None, message)
            };

            // 原子操作：状态更新 + 余额写入（可选）+ 记录创建放在同一事务中
            let final_message = sanitize_user_message(&final_message);
            let run = db::create_run_with_status_update_and_balance(
                db,
                account_id,
                &status,
                Some(&final_message),
                Some(duration_ms),
                triggered_by,
                raw_response.as_deref(),
                balance_to_store,
            )
            .await?;

            // 跨实例每日上限兜底：进程内单飞锁与计数预检只覆盖单进程（M5）。
            // 多实例共享同一 SQLite 时，两个实例可能同时通过预检各自签到；SQLite
            // 单写者保证“插入后计数复核”原子可见，超限则撤销本条记录并按 skipped
            // 返回，避免今日真实尝试数突破 maxAttemptsPerDay。代价：竞态窗口内
            // 可能多发一次到站点的网络签到，但本地不会超限计数。
            if let Some(s) = settings {
                if db::is_real_attempt(&run.status) {
                    let today_runs = db::count_runs_today(db, account_id).await?;
                    if today_runs > s.max_attempts_per_day.max(1) {
                        let msg = format!("已达到今日最大尝试次数 ({})", s.max_attempts_per_day);
                        tracing::warn!(
                            account_id = %account_id,
                            today_runs,
                            "跨实例竞态：撤销超出每日上限的签到记录"
                        );
                        let _ = db::delete_run(db, &run.id).await;
                        return Ok(skipped_run(
                            account_id,
                            &msg,
                            triggered_by,
                            start,
                            "skipped",
                        ));
                    }
                }
            }

            handle_notifications(db, &account, &status, &final_message, notification_balance).await;
            Ok(run)
        }
        Err(e) => {
            let detail = e.to_string();
            let msg = sanitize_user_message(&e.user_message());
            tracing::warn!(account_id = %account_id, error = %detail, "签到执行失败");
            let run = db::create_run_with_status_update(
                db,
                account_id,
                "failed",
                Some(&msg),
                Some(duration_ms),
                triggered_by,
                None,
            )
            .await?;
            handle_notifications(
                db,
                &account,
                "failed",
                &msg,
                account.last_balance.map(|quota| quota / QUOTA_PER_USD),
            )
            .await;
            Ok(run)
        }
    }
}

async fn execute_new_api_checkin(
    account: &CheckinAccount,
    profile: &BrowserProfile,
) -> Result<(String, String, Option<String>)> {
    // access_token 与 cookie 均可选，按实际配置传递（参考 Next.js runProvider）
    let access_token = account
        .access_token_enc
        .as_ref()
        .map(|t| decrypt(t))
        .transpose()?;
    let cookie = account
        .cookie_enc
        .as_ref()
        .map(|c| decrypt(c))
        .transpose()?;

    new_api::checkin(
        &account.base_url,
        account.user_id.as_deref(),
        access_token.as_deref(),
        cookie.as_deref(),
        profile,
    )
    .await
}

async fn execute_anyrouter_checkin(
    account: &CheckinAccount,
    profile: &BrowserProfile,
) -> Result<(String, String, Option<String>)> {
    super::validate_custom_checkin_url(
        &account.site_type,
        &account.base_url,
        account.custom_checkin_url.as_deref(),
    )?;
    let cookie = if let Some(enc) = &account.cookie_enc {
        Some(decrypt(enc)?)
    } else {
        None
    };

    anyrouter::checkin(
        &account.base_url,
        account.user_id.as_deref(),
        cookie.as_deref(),
        account.custom_checkin_url.as_deref(),
        profile,
    )
    .await
}

async fn execute_x666_checkin(
    account: &CheckinAccount,
    profile: &BrowserProfile,
) -> Result<(String, String, Option<String>)> {
    super::validate_custom_checkin_url(
        &account.site_type,
        &account.base_url,
        account.custom_checkin_url.as_deref(),
    )?;
    let cookie = if let Some(enc) = &account.cookie_enc {
        decrypt(enc)?
    } else {
        return Err(AppError::Validation("必须填写 cookie".into()));
    };

    x666::checkin(
        &account.base_url,
        &cookie,
        account.custom_checkin_url.as_deref(),
        profile,
    )
    .await
}

/// 查询账户余额（quota），供签到成功后刷新使用（参考 Next.js runner.ts fetchAccountBalance）。
/// - x666: 仅 cookie
/// - arrouter: userId + cookie（不传 access_token）
/// - new-api 及其他: userId + access_token + cookie
pub async fn fetch_account_balance(
    account: &CheckinAccount,
    profile: &BrowserProfile,
) -> Result<f64> {
    match account.site_type.as_str() {
        "x666" => {
            let enc = account
                .cookie_enc
                .as_ref()
                .ok_or_else(|| AppError::Validation("未配置 cookie".into()))?;
            let cookie = decrypt(enc)?;
            x666::fetch_balance(Some(&account.base_url), Some(&cookie), profile)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))
        }
        "anyrouter" => {
            let cookie = account
                .cookie_enc
                .as_ref()
                .map(|c| decrypt(c))
                .transpose()?;
            // anyrouter 余额查询不带 access_token，仅 cookie（与 Next.js 对齐）
            anyrouter::fetch_balance(
                &account.base_url,
                account.user_id.as_deref(),
                None,
                cookie.as_deref(),
                profile,
            )
            .await
            .map_err(|e| AppError::Internal(e.to_string()))
        }
        _ => {
            // new-api 及其他类型
            let access_token = account
                .access_token_enc
                .as_ref()
                .map(|t| decrypt(t))
                .transpose()?;
            let cookie = account
                .cookie_enc
                .as_ref()
                .map(|c| decrypt(c))
                .transpose()?;
            new_api::fetch_balance(
                &account.base_url,
                account.user_id.as_deref(),
                access_token.as_deref(),
                cookie.as_deref(),
                profile,
            )
            .await
            .map_err(|e| AppError::Internal(e.to_string()))
        }
    }
}

/// 构造一个“已跳过”的签到结果：不落库、不计入每日尝试上限、不触发通知。
/// 用于抢占、账户禁用、二次重检拦截、达到每日上限等非真实尝试路径（M6），
/// 与批量/定时前置跳过（status='skipped' 但不写记录）保持同一语义。
fn skipped_run(
    account_id: &str,
    message: &str,
    triggered_by: &str,
    start: Instant,
    status: &str,
) -> CheckinRun {
    CheckinRun {
        id: uuid::Uuid::new_v4().to_string(),
        account_id: account_id.to_string(),
        status: status.to_string(),
        message: Some(message.to_string()),
        duration_ms: Some(start.elapsed().as_millis().min(i64::MAX as u128) as i64),
        triggered_by: triggered_by.to_string(),
        raw_response: None,
        created_at: chrono::Utc::now(),
    }
}

async fn handle_notifications(
    db: &SqlitePool,
    account: &CheckinAccount,
    status: &str,
    message: &str,
    balance: Option<f64>,
) {
    let Some(owner_id) = account.owner_id.as_deref() else {
        return;
    };

    let consecutive_failures = if status == "failed" {
        match db::increment_failure_counter(db, &account.id).await {
            Ok(count) => count,
            Err(e) => {
                tracing::warn!(account_id = %account.id, error = %e, "更新失败计数失败");
                return;
            }
        }
    } else {
        if let Err(e) = db::reset_failure_counter(db, &account.id).await {
            tracing::warn!(account_id = %account.id, error = %e, "重置失败计数失败");
        }
        0
    };

    let configs = match db::list_notifications(db, owner_id).await {
        Ok(configs) => configs,
        Err(e) => {
            tracing::warn!(account_id = %account.id, owner_id = %owner_id, error = %e, "读取通知配置失败");
            return;
        }
    };

    if configs.is_empty() {
        return;
    }

    let payload = crate::services::notification::NotificationPayload {
        account_name: account.name.clone(),
        site_type: account.site_type.clone(),
        base_url: account.base_url.clone(),
        status: status.to_string(),
        message: message.to_string(),
        balance,
        consecutive_failures,
    };

    let mut sent_any = false;
    for config in configs {
        if !crate::services::notification::should_notify(&config, &payload) {
            continue;
        }

        match crate::services::notification::send_notification(&config, &payload).await {
            Ok(()) => {
                sent_any = true;
                tracing::info!(
                    account_id = %account.id,
                    notify_type = %config.notify_type,
                    "签到通知已发送"
                );
            }
            Err(e) => {
                tracing::warn!(
                    account_id = %account.id,
                    notify_type = %config.notify_type,
                    error = %e,
                    "签到通知发送失败"
                );
            }
        }
    }

    if sent_any {
        if let Err(e) = db::update_last_notified(db, &account.id).await {
            tracing::warn!(account_id = %account.id, error = %e, "更新通知时间失败");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{skip_reason_for_batch, BatchSkipReason};
    use crate::models::{CheckinAccount, CheckinSetting};
    use chrono::{Duration, Utc};

    fn account() -> CheckinAccount {
        let now = Utc::now();
        CheckinAccount {
            id: "account-1".into(),
            name: "测试账户".into(),
            site_type: "new-api".into(),
            base_url: "https://example.com".into(),
            user_id: None,
            owner_id: Some("user-1".into()),
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

    fn settings() -> CheckinSetting {
        CheckinSetting {
            id: "global".into(),
            enabled: true,
            schedule_cron: vec![],
            retry_enabled: true,
            max_attempts_per_day: 3,
            batch_delay_min: 0,
            batch_delay_max: 0,
            scheduled_delay_min: 0,
            scheduled_delay_max: 0,
            cleanup_keep_latest: 100,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn batch_skip_reason_exposes_user_facing_status_and_message() {
        assert_eq!(BatchSkipReason::AccountDisabled.status(), "skipped");
        assert_eq!(BatchSkipReason::AccountDisabled.message(), "账户已禁用");
        assert_eq!(BatchSkipReason::AlreadyChecked.status(), "already_checked");
        assert_eq!(BatchSkipReason::AlreadyChecked.message(), "今日已签到");
        assert_eq!(BatchSkipReason::RetryDisabled.status(), "skipped");
        assert_eq!(BatchSkipReason::RetryDisabled.message(), "重试已关闭");
    }

    #[test]
    fn batch_skip_reason_keeps_today_and_retry_rules_consistent() {
        let today = crate::business_time::today();
        let setting = settings();

        let mut disabled = account();
        disabled.enabled = false;
        assert_eq!(
            skip_reason_for_batch(&disabled, &setting, today),
            Some(BatchSkipReason::AccountDisabled)
        );

        let mut already_checked = account();
        already_checked.last_status = Some("success".into());
        already_checked.last_run_at = Some(Utc::now());
        assert_eq!(
            skip_reason_for_batch(&already_checked, &setting, today),
            Some(BatchSkipReason::AlreadyChecked)
        );

        let mut retry_disabled = account();
        retry_disabled.last_status = Some("failed".into());
        retry_disabled.last_run_at = Some(Utc::now());
        retry_disabled.retry_enabled = false;
        assert_eq!(
            skip_reason_for_batch(&retry_disabled, &setting, today),
            Some(BatchSkipReason::RetryDisabled)
        );

        let mut yesterday_failed = account();
        yesterday_failed.last_status = Some("failed".into());
        yesterday_failed.last_run_at = Some(Utc::now() - Duration::days(2));
        assert_eq!(
            skip_reason_for_batch(&yesterday_failed, &setting, today),
            None
        );
    }
}
