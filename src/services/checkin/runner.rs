use sqlx::SqlitePool;
use std::time::Instant;
use crate::{
    models::{CheckinAccount, CheckinRun},
    error::{Result, AppError},
    crypto::decrypt,
    db,
};
use super::providers::{new_api, anyrouter, x666};

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
    
    let result = match account.site_type.as_str() {
        "new-api" => execute_new_api_checkin(&account).await,
        "anyrouter" => execute_anyrouter_checkin(&account).await,
        "x666" => execute_x666_checkin(&account).await,
        _ => Err(AppError::Validation(format!("Unsupported site type: {}", account.site_type))),
    };
    
    let duration_ms = start.elapsed().as_millis() as i32;
    
    match result {
        Ok((status, message, raw_response)) => {
            db::update_account_status(db, account_id, &status, Some(&message)).await?;
            db::create_run(db, account_id, &status, Some(&message), Some(duration_ms), triggered_by, raw_response.as_deref()).await
        }
        Err(e) => {
            let msg = e.to_string();
            db::update_account_status(db, account_id, "failed", Some(&msg)).await?;
            db::create_run(db, account_id, "failed", Some(&msg), Some(duration_ms), triggered_by, None).await
        }
    }
}

async fn execute_new_api_checkin(account: &CheckinAccount) -> Result<(String, String, Option<String>)> {
    let token = if let Some(enc) = &account.access_token_enc {
        decrypt(enc)?
    } else {
        return Err(AppError::Validation("Access token required".into()));
    };
    
    new_api::checkin(&account.base_url, &token, account.user_id.as_deref()).await
}

async fn execute_anyrouter_checkin(account: &CheckinAccount) -> Result<(String, String, Option<String>)> {
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
    )
    .await
}

async fn execute_x666_checkin(account: &CheckinAccount) -> Result<(String, String, Option<String>)> {
    let cookie = if let Some(enc) = &account.cookie_enc {
        decrypt(enc)?
    } else {
        return Err(AppError::Validation("Cookie required".into()));
    };
    
    x666::checkin(&account.base_url, &cookie, account.custom_checkin_url.as_deref()).await
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
