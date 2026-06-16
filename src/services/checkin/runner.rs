use sqlx::SqlitePool;
use std::time::Instant;
use chrono::Local;
use crate::{
    models::{CheckinAccount, CheckinRun, CheckinSetting},
    error::{Result, AppError},
    crypto::decrypt,
    db,
};
use super::BrowserProfile;
use super::providers::{new_api, anyrouter, x666};

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
) -> Result<CheckinRun> {
    let start = Instant::now();
    
    let account = db::find_account_by_id(db, account_id)
        .await?
        .ok_or(AppError::NotFound)?;
    
    if !account.enabled {
        return create_failed_run(db, account_id, "Account disabled", triggered_by, start).await;
    }
    
    // 防判定：每次签到使用随机 UA，降低多账户同 IP + 同 UA 的关联指纹。
    let profile = super::random_browser_profile();

    let result = match account.site_type.as_str() {
        "new-api" => execute_new_api_checkin(&account, profile).await,
        "arrouter" => execute_arrouter_checkin(&account, profile).await,
        "x666" => execute_x666_checkin(&account, profile).await,
        _ => Err(AppError::Validation(format!("Unsupported site type: {}", account.site_type))),
    };
    
    let duration_ms = start.elapsed().as_millis() as i32;
    
    match result {
        Ok((status, message, raw_response)) => {
            // 签到成功或今日已签时刷新余额（参考 Next.js runner.ts）
            // 余额刷新失败不影响签到结果，仅在消息中追加提示
            let final_message = if status.as_str() == "success" || status.as_str() == "already_checked" {
                match fetch_account_balance(&account, profile).await {
                    Ok(quota) => {
                        if let Err(e) = db::update_account_balance(db, account_id, quota).await {
                            tracing::warn!(account_id = %account_id, error = %e, "签到后余额写库失败");
                        }
                        message
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        tracing::warn!(account_id = %account_id, error = %msg, "签到后余额刷新失败");
                        format!("{}；余额刷新失败：{}", message, msg)
                    }
                }
            } else {
                message
            };

            db::update_account_status(db, account_id, &status, Some(&final_message)).await?;
            db::create_run(db, account_id, &status, Some(&final_message), Some(duration_ms), triggered_by, raw_response.as_deref()).await
        }
        Err(e) => {
            let msg = e.to_string();
            db::update_account_status(db, account_id, "failed", Some(&msg)).await?;
            db::create_run(db, account_id, "failed", Some(&msg), Some(duration_ms), triggered_by, None).await
        }
    }
}

async fn execute_new_api_checkin(account: &CheckinAccount, profile: &BrowserProfile) -> Result<(String, String, Option<String>)> {
    // access_token 与 cookie 均可选，按实际配置传递（参考 Next.js runProvider）
    let access_token = account.access_token_enc.as_ref()
        .map(|t| decrypt(t))
        .transpose()?;
    let cookie = account.cookie_enc.as_ref()
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

async fn execute_arrouter_checkin(account: &CheckinAccount, profile: &BrowserProfile) -> Result<(String, String, Option<String>)> {
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

async fn execute_x666_checkin(account: &CheckinAccount, profile: &BrowserProfile) -> Result<(String, String, Option<String>)> {
    let cookie = if let Some(enc) = &account.cookie_enc {
        decrypt(enc)?
    } else {
        return Err(AppError::Validation("Cookie required".into()));
    };

    x666::checkin(&account.base_url, &cookie, account.custom_checkin_url.as_deref(), profile).await
}

/// 查询账户余额（quota），供签到成功后刷新使用（参考 Next.js runner.ts fetchAccountBalance）。
/// - x666: 仅 cookie
/// - arrouter: userId + cookie（不传 access_token）
/// - new-api 及其他: userId + access_token + cookie
async fn fetch_account_balance(account: &CheckinAccount, profile: &BrowserProfile) -> Result<f64> {
    match account.site_type.as_str() {
        "x666" => {
            let enc = account.cookie_enc.as_ref()
                .ok_or_else(|| AppError::Validation("Cookie not configured".into()))?;
            let cookie = decrypt(enc)?;
            x666::fetch_balance(Some(&cookie), profile)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))
        }
        "arrouter" => {
            let cookie = account.cookie_enc.as_ref()
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
            let access_token = account.access_token_enc.as_ref()
                .map(|t| decrypt(t))
                .transpose()?;
            let cookie = account.cookie_enc.as_ref()
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
    let duration_ms = start.elapsed().as_millis() as i32;
    db::create_run(db, account_id, "failed", Some(message), Some(duration_ms), triggered_by, None).await
}
