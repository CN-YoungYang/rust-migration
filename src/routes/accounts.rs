use axum::{
    extract::{State, Path, Extension},
    Json,
};
use std::sync::Arc;
use serde_json::{json, Value};
use crate::{
    AppState,
    models::{CheckinAccount, CreateAccountRequest, UpdateAccountRequest},
    error::Result,
    crypto::{encrypt},
    db,
};

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
    let masked: Vec<Value> = accounts.into_iter().map(|acc| {
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
    }).collect();
    
    Ok(Json(masked))
}

pub async fn get(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<crate::models::AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let acc = db::find_account_by_id(&state.db, &id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    
    Ok(Json(json!({
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
    })))
}

pub async fn create(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<Json<CheckinAccount>> {
    let access_token_enc = payload.access_token.as_ref().map(|t| encrypt(t)).transpose()?;
    let cookie_enc = payload.cookie.as_ref().map(|c| encrypt(c)).transpose()?;
    
    let account = db::create_account(
        &state.db,
        &payload.name,
        &payload.site_type,
        &payload.base_url,
        payload.user_id.as_deref(),
        &payload.auth_type,
        access_token_enc.as_deref(),
        cookie_enc.as_deref(),
        payload.custom_checkin_url.as_deref(),
        payload.enabled.unwrap_or(true),
        payload.retry_enabled.unwrap_or(true),
        &user.id,
    ).await?;
    
    Ok(Json(account))
}
pub async fn update(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAccountRequest>,
) -> Result<Json<Value>> {
    let access_token_enc = payload.access_token.as_ref().map(|t| encrypt(t)).transpose()?;
    let cookie_enc = payload.cookie.as_ref().map(|c| encrypt(c)).transpose()?;
    let existing = db::find_account_by_id(&state.db, &id).await?.ok_or(crate::error::AppError::NotFound)?;
    check_account_ownership(&existing, &user)?;
        
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
    
    Ok(Json(json!({ "success": true })))
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
    State(state): State<Arc<AppState>>, Extension(_user): Extension<crate::models::AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    use crate::services::checkin::providers::{new_api, x666};
    use crate::crypto::decrypt_secret;
    
    let account = db::find_account_by_id(&state.db, &id).await?
        .ok_or(crate::error::AppError::NotFound)?;
    
    let quota = if account.site_type == "x666" {
        let cookie = account.cookie_enc.as_ref()
            .and_then(|c| decrypt_secret(c).ok());
        x666::fetch_balance(cookie.as_deref()).await
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
    } else {
        let access_token = account.access_token_enc.as_ref()
            .and_then(|t| decrypt_secret(t).ok());
        let cookie = account.cookie_enc.as_ref()
            .and_then(|c| decrypt_secret(c).ok());
        
        new_api::fetch_balance(
            &account.base_url,
            account.user_id.as_deref(),
            access_token.as_deref(),
            cookie.as_deref()
        ).await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
    };
    
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
