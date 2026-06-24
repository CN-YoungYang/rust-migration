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

/// SSRF 防护：检查 URL 是否指向内网/私有地址。
/// 拒绝 127.0.0.0/8、10.0.0.0/8、172.16.0.0/12、192.168.0.0/16、
/// 169.254.0.0/16（云元数据）、0.0.0.0、localhost、::1 等。
fn is_private_url(url: &str) -> bool {
    // 提取 host 部分（reqwest 依赖已包含 url crate）
    let host = match reqwest::Url::parse(url) {
        Ok(u) => u.host_str().unwrap_or("").to_lowercase(),
        Err(_) => return true, // 无法解析的 URL 视为不安全
    };

    // 域名检查
    if host == "localhost" || host.ends_with(".local") || host.ends_with(".internal") {
        return true;
    }

    // IPv6 回环
    if host == "::1" || host == "[::1]" {
        return true;
    }

    // IPv4 解析
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        let octets: [u8; 4] = ip.octets();
        return match octets {
            [127, ..] => true,                              // 127.0.0.0/8
            [10, ..] => true,                               // 10.0.0.0/8
            [172, b, ..] if (16..=31).contains(&b) => true, // 172.16.0.0/12
            [192, 168, ..] => true,                         // 192.168.0.0/16
            [169, 254, ..] => true,                         // 169.254.0.0/16 (云元数据)
            [0, 0, 0, 0] => true,                           // 0.0.0.0
            _ => false,
        };
    }

    // IPv6 私有地址
    if let Ok(ip) = host.trim_matches(|c| c == '[' || c == ']').parse::<std::net::Ipv6Addr>() {
        if ip.is_loopback() || ip.is_unspecified() {
            return true;
        }
        // fc00::/7 (ULA)
        let segments = ip.segments();
        if (segments[0] & 0xfe00) == 0xfc00 {
            return true;
        }
        // fe80::/10 (link-local)
        if (segments[0] & 0xffc0) == 0xfe80 {
            return true;
        }
    }

    false
}

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
        "note": acc.note,
        "lastBalance": acc.last_balance,
        "lastBalanceAt": acc.last_balance_at,
        "lastStatus": acc.last_status,
        "lastMessage": acc.last_message,
        "lastRunAt": acc.last_run_at,
        "createdAt": acc.created_at,
        "updatedAt": acc.updated_at,
    })
}

/// 带今日签到次数的账户 JSON（供列表接口使用）
fn account_to_json_with_runs(acc: &CheckinAccount, owner_name: Option<&str>, today_runs: i32) -> Value {
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
        "note": acc.note,
        "lastBalance": acc.last_balance,
        "lastBalanceAt": acc.last_balance_at,
        "lastStatus": acc.last_status,
        "lastMessage": acc.last_message,
        "lastRunAt": acc.last_run_at,
        "todayRuns": today_runs,
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
    let filter_site_type = params.get("siteType").map(|s| s.as_str());
    let filter_enabled = params.get("enabled").and_then(|s| s.parse::<bool>().ok());
    let filter_last_status = params.get("lastStatus").map(|s| s.as_str());
    let filter_keyword = params.get("keyword").map(|s| s.as_str());
    let limit: i32 = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(500).min(1000);
    let offset: i32 = params.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0).max(0);

    let owner_id = if user.role == "ADMIN" || user.role == "SUPER_ADMIN" {
        filter_user_id.map(|s| s.as_str())
    } else {
        Some(user.id.as_str())
    };

    let accounts = db::list_accounts_filtered(
        &state.db,
        &db::AccountFilter {
            owner_id: owner_id.map(|s| s.to_string()),
            site_type: filter_site_type.map(|s| s.to_string()),
            enabled: filter_enabled,
            last_status: filter_last_status.map(|s| s.to_string()),
            keyword: filter_keyword.map(|s| s.to_string()),
            limit,
            offset,
        },
    ).await?;

    // 轻量查询：只取 id + username，避免拉取 passwordHash 等无关字段
    let owner_map = db::list_user_id_name_map(&state.db).await?;
    // 批量查询今日各账户签到次数，用于前端判断是否达到每日上限
    let today_counts = db::count_runs_today_batch(&state.db).await.unwrap_or_default();

    let masked: Vec<Value> = accounts
        .iter()
        .map(|acc| {
            let owner_name = acc
                .owner_id
                .as_deref()
                .and_then(|id| owner_map.get(id))
                .map(|s| s.as_str());
            let today_runs = today_counts.get(&acc.id).copied().unwrap_or(0);
            account_to_json_with_runs(acc, owner_name, today_runs)
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
    if is_private_url(&payload.base_url) {
        return Err(crate::error::AppError::Validation("站点地址不能指向内网/私有地址（SSRF 防护）".into()));
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
        &db::CreateAccountRequest {
            name: payload.name.clone(),
            site_type: payload.site_type.clone(),
            base_url: payload.base_url.clone(),
            user_id: payload.user_id.clone(),
            auth_type: auth_type.to_string(),
            access_token_enc,
            cookie_enc,
            custom_checkin_url: payload.custom_checkin_url.clone(),
            enabled: payload.enabled.unwrap_or(true),
            retry_enabled: payload.retry_enabled.unwrap_or(true),
            owner_id: user.id.clone(),
            note: payload.note.clone(),
        },
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

    // SSRF 防护：如果修改了 base_url，检查是否指向内网
    if let Some(ref new_base) = payload.base_url {
        if !new_base.starts_with("http://") && !new_base.starts_with("https://") {
            return Err(crate::error::AppError::Validation("站点地址必须以 http:// 或 https:// 开头".into()));
        }
        if is_private_url(new_base) {
            return Err(crate::error::AppError::Validation("站点地址不能指向内网/私有地址（SSRF 防护）".into()));
        }
    }

    // Validate based on site type (use existing site_type since it can't be changed)
    let site_type = &existing.site_type;
    
    // Check if user_id is being cleared for anyrouter
    if site_type == "anyrouter" {
        let has_user_id = match &payload.user_id {
            Some(Some(_)) => true,        // 设置了新值
            Some(None) => false,           // 明确清空
            None => existing.user_id.is_some(), // 未传，看原值
        };
        if !has_user_id {
            return Err(crate::error::AppError::Validation("AnyRouter 必须填写 userId".into()));
        }
    }

    // Check if cookie is being cleared for anyrouter/x666
    if site_type == "anyrouter" || site_type == "x666" {
        let has_cookie = match &payload.cookie {
            Some(Some(_)) => true,
            Some(None) => false,
            None => existing.cookie_enc.is_some(),
        };
        if !has_cookie {
            return Err(crate::error::AppError::Validation(
                format!("{} 必须填写 cookie", site_type)
            ));
        }
    }

    // Check if access_token is being cleared for new-api with access_token auth
    if site_type != "anyrouter" && site_type != "x666" && existing.auth_type == "access_token" {
        let has_token = match &payload.access_token {
            Some(Some(_)) => true,
            Some(None) => false,
            None => existing.access_token_enc.is_some(),
        };
        if !has_token {
            return Err(crate::error::AppError::Validation(
                "认证方式为 access_token 时必须填写 accessToken".into()
            ));
        }
    }

    // 三态处理加密字段：None=保持原值, Some(None)=清空, Some(Some(v))=加密后存储
    let access_token_enc: Option<Option<String>> = match &payload.access_token {
        None => None,
        Some(None) => Some(None),
        Some(Some(t)) => Some(Some(encrypt(t)?)),
    };
    let cookie_enc: Option<Option<String>> = match &payload.cookie {
        None => None,
        Some(None) => Some(None),
        Some(Some(c)) => Some(Some(encrypt(c)?)),
    };

    let updated = db::update_account(
        &state.db,
        &id,
        &db::UpdateAccountRequest {
            name: payload.name.clone(),
            base_url: payload.base_url.clone(),
            user_id: payload.user_id.clone(),
            access_token_enc,
            cookie_enc,
            custom_checkin_url: payload.custom_checkin_url.clone(),
            enabled: payload.enabled,
            retry_enabled: payload.retry_enabled,
            note: payload.note.clone(),
        },
    ).await?;

    // update_account 已返回更新后的账户，无需再次查询
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
