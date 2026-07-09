use super::providers::{anyrouter, new_api, x666};
use super::BrowserProfile;
use crate::{
    crypto::decrypt,
    db,
    error::{AppError, Result},
    models::{CheckinAccount, CheckinRun, CheckinSetting},
};
use chrono::Local;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

// ── 防并发签到：同一账户同时只能有一个签到任务在执行 ──────────────────────
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
/// 与 `scheduler` 内联判断保持一致：
/// - 已禁用 -> account_disabled
/// - 今日已 success/already_checked -> already_succeeded_today
/// - 今日已尝试且（全局或账户）关闭重试 -> retry_disabled
pub fn skip_reason_for_batch(
    account: &CheckinAccount,
    settings: &CheckinSetting,
    today_local: chrono::NaiveDate,
) -> Option<&'static str> {
    if !account.enabled {
        return Some("account_disabled");
    }

    if let Some(last_run) = account.last_run_at {
        let last_run_local = last_run.with_timezone(&Local);
        if last_run_local.date_naive() == today_local {
            if let Some(status) = &account.last_status {
                if status == "success" || status == "already_checked" {
                    return Some("already_succeeded_today");
                }
            }

            // 今日已尝试且未成功：仅当全局和账户都允许重试时才继续
            if !settings.retry_enabled || !account.retry_enabled {
                return Some("retry_disabled");
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

    // 防并发：同一账户同时只能有一个签到任务（定时/手动/批量共用）
    let _guard = match InFlightGuard::try_acquire(account_id) {
        Some(g) => g,
        None => {
            return create_failed_run(
                db,
                account_id,
                "该账户正在签到中，请稍后再试",
                triggered_by,
                start,
            )
            .await;
        }
    };

    let account = db::find_account_by_id(db, account_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if !account.enabled {
        return create_failed_run(db, account_id, "账户已禁用", triggered_by, start).await;
    }

    // TOCTOU 重检查：用刚从 DB 取到的最新账户状态再做一次 skip 判断，
    // 避免调用方的 skip_reason_for_batch 与实际执行之间账户状态已变化。
    if let Some(s) = settings {
        let today_local = chrono::Local::now().date_naive();
        if let Some(reason) = skip_reason_for_batch(&account, s, today_local) {
            return create_failed_run(db, account_id, reason, triggered_by, start).await;
        }
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
            // 签到成功或今日已签时刷新余额（参考 Next.js runner.ts）
            // 余额刷新失败了不影响签到结果，仅在消息中追加提示。
            // 余额刷新为网络请求，无法并入 DB 事务；但其写库与状态更新、记录创建
            // 通过 create_run_with_status_update_and_balance 在同一事务原子提交，
            // 避免崩溃时出现"余额已更新但无签到记录"的部分写入。
            let mut notification_balance = account.last_balance;
            let (balance_to_store, final_message) = if status.as_str() == "success"
                || status.as_str() == "already_checked"
            {
                match fetch_account_balance(&account, profile).await {
                    Ok(quota) => {
                        notification_balance = Some(quota);
                        (Some(quota), message)
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        tracing::warn!(account_id = %account_id, error = %msg, "签到后余额刷新失败");
                        (None, format!("{}；余额刷新失败：{}", message, msg))
                    }
                }
            } else {
                (None, message)
            };

            // 原子操作：状态更新 + 余额写入（可选）+ 记录创建放在同一事务中
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
            handle_notifications(db, &account, &status, &final_message, notification_balance).await;
            Ok(run)
        }
        Err(e) => {
            let msg = e.to_string();
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

async fn create_failed_run(
    db: &SqlitePool,
    account_id: &str,
    message: &str,
    triggered_by: &str,
    start: Instant,
) -> Result<CheckinRun> {
    let duration_ms = start.elapsed().as_millis().min(i64::MAX as u128) as i64;
    db::create_run(
        db,
        account_id,
        "failed",
        Some(message),
        Some(duration_ms),
        triggered_by,
        None,
    )
    .await
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
