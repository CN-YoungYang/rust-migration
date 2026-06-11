use axum::{
    extract::{State, Path},
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

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Value>>> {
    let accounts = db::list_accounts(&state.db).await?;
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
    State(state): State<Arc<AppState>>,
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
    State(state): State<Arc<AppState>>,
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
    ).await?;
    
    Ok(Json(account))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAccountRequest>,
) -> Result<Json<Value>> {
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
    
    Ok(Json(json!({ "success": true })))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    db::delete_account(&state.db, &id).await?;
    Ok(Json(json!({ "success": true })))
}
