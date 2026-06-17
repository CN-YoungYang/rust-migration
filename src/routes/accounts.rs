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

fn account_to_json(acc: &CheckinAccount, owner_name: Option<&str>) -> Value {
    json!({
        "id": acc.id,
        "name": acc.name,
        "siteType": acc.site_type,
        "baseUrl": acc.base_url,
        "userId": acc.user_id,
        "ownerId": acc.owner_id,
        "ownerName": owner_name,
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

// Look up the username of an account's owner so the UI can group accounts by user.
async fn resolve_owner_name(state: &AppState, owner_id: &Option<String>) -> Result<Option<String>> {
    match owner_id.as_deref() {
        Some(id) => Ok(db::find_user_by_id(&state.db, id).await?.map(|u| u.username)),
        None => Ok(None),
    }
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

    // Build an ownerId -> username map so the UI can group accounts by their owner.
    let users = db::list_users(&state.db).await?;
    let owner_map: std::collections::HashMap<&str, &str> = users
        .iter()
        .map(|u| (u.id.as_str(), u.username.as_str()))
        .collect();

    let masked: Vec<Value> = accounts
        .iter()
        .map(|acc| {
            let owner_name = acc
                .owner_id
                .as_deref()
                .and_then(|id| owner_map.get(id))
                .copied();
            account_to_json(acc, owner_name)
        })
        .collect();

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

    let owner_name = resolve_owner_name(&state, &acc.owner_id).await?;
    Ok(Json(account_to_json(&acc, owner_name.as_deref())))
}

pub async fn create(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<Json<Value>> {
    // 基础校验
    let valid_site_types = ["new-api", "anyrouter", "x666"];
    if !valid_site_types.contains(&payload.site_type.as_str()) {
        return Err(crate::error::AppError::Validation(
            format!("不支持的站点类型: {}，可选: {:?}", payload.site_type, valid_site_types)
        ));
    }
    if !payload.base_url.starts_with("http://") && !payload.base_url.starts_with("https://") {
        return Err(crate::error::AppError::Validation("站点地址必须以 http:// 或 https:// 开头".into()));
    }
    if payload.name.trim().is_empty() {
        return Err(crate::error::AppError::Validation("账户名称不能为空".into()));
    }
    if payload.name.len() > 200 {
        return Err(crate::error::AppError::Validation("账户名称不能超过 200 字符".into()));
    }

    // Validate required fields based on site type
    if payload.site_type == "anyrouter" && payload.user_id.is_none() {
        return Err(crate::error::AppError::Validation("AnyRouter 必须填写 userId".into()));
    }

    if (payload.site_type == "anyrouter" || payload.site_type == "x666") && payload.cookie.is_none() {
        return Err(crate::error::AppError::Validation(
            format!("{} 必须填写 cookie", payload.site_type)
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
            "认证方式为 access_token 时必须填写 accessToken".into()
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

    // The new account is owned by the current user, so we can reuse their username directly.
    Ok(Json(account_to_json(&acc, Some(&user.username))))
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
            return Err(crate::error::AppError::Validation("AnyRouter 必须填写 userId".into()));
        }
    }
    
    // Check if cookie is being cleared for anyrouter/x666
    if site_type == "anyrouter" || site_type == "x666" {
        let has_cookie = payload.cookie.is_some() || existing.cookie_enc.is_some();
        if !has_cookie {
            return Err(crate::error::AppError::Validation(
                format!("{} 必须填写 cookie", site_type)
            ));
        }
    }
    
    // Check if access_token is being cleared for new-api with access_token auth
    if site_type != "anyrouter" && site_type != "x666" && existing.auth_type == "access_token" {
        let has_token = payload.access_token.is_some() || existing.access_token_enc.is_some();
        if !has_token {
            return Err(crate::error::AppError::Validation(
                "认证方式为 access_token 时必须填写 accessToken".into()
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
    let owner_name = resolve_owner_name(&state, &updated.owner_id).await?;
    Ok(Json(account_to_json(&updated, owner_name.as_deref())))
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
    use crate::services::checkin::{runner, random_browser_profile};

    // 防判定：余额刷新同样使用随机浏览器指纹，避免和签到请求指纹不一致被 WAF 关联。
    let profile = random_browser_profile();

    let account = db::find_account_by_id(&state.db, &id).await?
        .ok_or(crate::error::AppError::NotFound)?;

    check_account_ownership(&account, &user)?;

    tracing::info!(
        account_id = %id,
        site_type = %account.site_type,
        "Refreshing balance"
    );

    // 复用 runner 的余额查询逻辑，避免代码重复
    let quota = runner::fetch_account_balance(&account, profile).await
        .map_err(|e| {
            tracing::error!("fetch_balance error: {}", e);
            e
        })?;

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
