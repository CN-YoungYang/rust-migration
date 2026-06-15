use axum::{
    extract::{State, Path, Extension},
    Json,
};
use std::sync::Arc;
use serde_json::{json, Value};
use crate::{
    AppState,
    models::{CreateAccountRequest, CheckinAccount, UpdateAccountRequest},
    error::Result,
    crypto::{encrypt},
    db,
};

fn account_to_json(acc: &CheckinAccount) -> Value {
    json!({
        "id": acc.id,
        "name": acc.name,
        "siteType": acc.site_type,
        "baseUrl": acc.base_url,
        "userId": acc.user_id,
        "authType": acc.auth_type,
        "accessTokenMasked": acc.access_token_enc.as_ref().map(|_| "****"),
        "cookieMasked": acc.cookie_enc.as_ref().map(|_| "****"),
        "customCheckinUrl": acc.custom_checkin_url,
        "enabled": acc.enabled,
        "retryEnabled": acc.retry_enabled,
        "lastBalance": acc.last_balance,
        "lastBalanceAt": acc.last_balance_at,
        "lastStatus": acc.last_status,
        "lastMessage": acc.last_message,
        "lastRunAt": acc.last_run_at,
        "createdAt": acc.created_at,
        "updatedAt": acc.updated_at,
    })
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<Value>>> {
    let filter_user_id = params.get("userId");
    
    let accounts = if user.role == "ADMIN" || user.role == "SUPER_ADMIN" {
        if let Some(uid) = filter_user_id {
            db::list_accounts_by_user(&state.db, uid).await?
        } else {
            db::list_accounts(&state.db).await?
        }
    } else {
        db::list_accounts_by_user(&state.db, &user.id).await?
    };
    let masked: Vec<Value> = accounts.iter().map(account_to_json).collect();

    Ok(Json(masked))
}

pub async fn get(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let acc = db::find_account_by_id(&state.db, &id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;

    check_account_ownership(&acc, &user)?;

    Ok(Json(account_to_json(&acc)))
}

pub async fn create(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<Json<Value>> {
    // Validate required fields based on site type
    if payload.site_type == "anyrouter" && payload.user_id.is_none() {
        return Err(crate::error::AppError::Validation("userId is required for AnyRouter".into()));
    }
    
    if (payload.site_type == "anyrouter" || payload.site_type == "x666") && payload.cookie.is_none() {
        return Err(crate::error::AppError::Validation(
            format!("cookie is required for {}", payload.site_type)
        ));
    }
    
    // Auto-adjust authType for anyrouter and x666
    let auth_type = if payload.site_type == "anyrouter" || payload.site_type == "x666" {
        "cookie".to_string()
    } else {
        payload.auth_type.clone()
    };
    
    if payload.site_type != "anyrouter" && payload.site_type != "x666" 
        && auth_type == "access_token" && payload.access_token.is_none() {
        return Err(crate::error::AppError::Validation(
            "accessToken is required when authType is access_token".into()
        ));
    }
    
    let access_token_enc = payload.access_token.as_ref().map(|t| encrypt(t)).transpose()?;
    let cookie_enc = payload.cookie.as_ref().map(|c| encrypt(c)).transpose()?;

    let acc = db::create_account(
        &state.db,
        &payload.name,
        &payload.site_type,
        &payload.base_url,
        payload.user_id.as_deref(),
        &auth_type,
        access_token_enc.as_deref(),
        cookie_enc.as_deref(),
        payload.custom_checkin_url.as_deref(),
        payload.enabled.unwrap_or(true),
        payload.retry_enabled.unwrap_or(true),
        &user.id,
    ).await?;

    Ok(Json(account_to_json(&acc)))
}
pub async fn update(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAccountRequest>,
) -> Result<Json<Value>> {
    let existing = db::find_account_by_id(&state.db, &id).await?.ok_or(crate::error::AppError::NotFound)?;
    check_account_ownership(&existing, &user)?;
    
    // Validate based on site type (use existing site_type since it can't be changed)
    let site_type = &existing.site_type;
    
    // Check if user_id is being cleared for anyrouter
    if site_type == "anyrouter" {
        let has_user_id = payload.user_id.as_ref().map(|s| !s.is_empty()).unwrap_or(false) 
            || existing.user_id.is_some();
        if !has_user_id {
            return Err(crate::error::AppError::Validation("userId is required for AnyRouter".into()));
        }
    }
    
    // Check if cookie is being cleared for anyrouter/x666
    if site_type == "anyrouter" || site_type == "x666" {
        let has_cookie = payload.cookie.is_some() || existing.cookie_enc.is_some();
        if !has_cookie {
            return Err(crate::error::AppError::Validation(
                format!("cookie is required for {}", site_type)
            ));
        }
    }
    
    // Check if access_token is being cleared for new-api with access_token auth
    if site_type != "anyrouter" && site_type != "x666" && existing.auth_type == "access_token" {
        let has_token = payload.access_token.is_some() || existing.access_token_enc.is_some();
        if !has_token {
            return Err(crate::error::AppError::Validation(
                "accessToken is required when authType is access_token".into()
            ));
        }
    }
    
    let access_token_enc = payload.access_token.as_ref().map(|t| encrypt(t)).transpose()?;
    let cookie_enc = payload.cookie.as_ref().map(|c| encrypt(c)).transpose()?;
        
    db::update_account(
        &state.db,
        &id,
        payload.name.as_deref(),
        payload.base_url.as_deref(),
        payload.user_id.as_deref(),
        access_token_enc.as_deref(),
        cookie_enc.as_deref(),
        payload.custom_checkin_url.as_deref(),
        payload.enabled,
        payload.retry_enabled,
    ).await?;

    let updated = db::find_account_by_id(&state.db, &id).await?.ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(account_to_json(&updated)))
}

pub async fn delete(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {

    let existing = db::find_account_by_id(&state.db, &id).await?.ok_or(crate::error::AppError::NotFound)?;
    check_account_ownership(&existing, &user)?;
    db::delete_account(&state.db, &id).await?;
    Ok(Json(json!({ "success": true })))
}

pub async fn refresh_balance(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    use crate::services::checkin::providers::{new_api, anyrouter, x666};
    use crate::crypto::decrypt_secret;

    let account = db::find_account_by_id(&state.db, &id).await?
        .ok_or(crate::error::AppError::NotFound)?;

    check_account_ownership(&account, &user)?;

    tracing::info!(
        account_id = %id,
        site_type = %account.site_type,
        "Refreshing balance"
    );

    let quota = if account.site_type == "x666" {
        let cookie = account.cookie_enc.as_ref()
            .ok_or_else(|| crate::error::AppError::Validation("Cookie not configured".into()))?;
        let cookie_decrypted = decrypt_secret(cookie)
            .map_err(|e| {
                tracing::error!("Failed to decrypt cookie: {}", e);
                crate::error::AppError::Internal("解密失败".to_string())
            })?;
        
        x666::fetch_balance(Some(&cookie_decrypted)).await
            .map_err(|e| {
                tracing::error!("X666 fetch_balance error: {}", e);
                crate::error::AppError::Internal(e.to_string())
            })?
    } else if account.site_type == "anyrouter" {
        let access_token = account.access_token_enc.as_ref()
            .map(|t| decrypt_secret(t))
            .transpose()
            .map_err(|e| {
                tracing::error!("Failed to decrypt access_token: {}", e);
                crate::error::AppError::Internal("解密失败".to_string())
            })?;
        let cookie = account.cookie_enc.as_ref()
            .map(|c| decrypt_secret(c))
            .transpose()
            .map_err(|e| {
                tracing::error!("Failed to decrypt cookie: {}", e);
                crate::error::AppError::Internal("解密失败".to_string())
            })?;
        
        anyrouter::fetch_balance(
            &account.base_url,
            account.user_id.as_deref(),
            access_token.as_deref(),
            cookie.as_deref()
        ).await
        .map_err(|e| {
            tracing::error!("AnyRouter fetch_balance error: {}", e);
            crate::error::AppError::Internal(e.to_string())
        })?
    } else {
        // new-api or other types
        let access_token = account.access_token_enc.as_ref()
            .map(|t| decrypt_secret(t))
            .transpose()
            .map_err(|e| {
                tracing::error!("Failed to decrypt access_token: {}", e);
                crate::error::AppError::Internal("解密失败".to_string())
            })?;
        let cookie = account.cookie_enc.as_ref()
            .map(|c| decrypt_secret(c))
            .transpose()
            .map_err(|e| {
                tracing::error!("Failed to decrypt cookie: {}", e);
                crate::error::AppError::Internal("解密失败".to_string())
            })?;
        
        new_api::fetch_balance(
            &account.base_url,
            account.user_id.as_deref(),
            access_token.as_deref(),
            cookie.as_deref()
        ).await
        .map_err(|e| {
            tracing::error!("New-API fetch_balance error: {}", e);
            crate::error::AppError::Internal(e.to_string())
        })?
    };
    
    tracing::info!(
        account_id = %id,
        quota = %quota,
        "Balance refreshed successfully"
    );
    
    db::update_account_balance(&state.db, &id, quota).await?;
    
    Ok(Json(json!({
        "success": true,
        "balance": quota
    })))
}
fn check_account_ownership(account: &crate::models::CheckinAccount, user: &crate::models::AppUser) -> Result<()> {
    if user.role == "ADMIN" || user.role == "SUPER_ADMIN" || account.owner_id.as_ref() == Some(&user.id) {
        Ok(())
    } else {
        Err(crate::error::AppError::Forbidden)
    }
}
