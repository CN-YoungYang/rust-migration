use crate::{
    crypto::hash_password,
    db,
    error::Result,
    models::{AppUser, UpdateUserRequest},
    AppState,
};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

// Check if current user can manage target user
fn check_admin_permission(current_user: &AppUser, target_user: &AppUser) -> Result<()> {
    // SUPER_ADMIN can manage anyone except other SUPER_ADMINs
    if current_user.role == "SUPER_ADMIN" {
        if target_user.role == "SUPER_ADMIN" {
            return Err(crate::error::AppError::Forbidden);
        }
        return Ok(());
    }

    // ADMIN can only manage USERs
    if current_user.role == "ADMIN" {
        if target_user.role == "USER" {
            return Ok(());
        }
        return Err(crate::error::AppError::Forbidden);
    }

    Err(crate::error::AppError::Forbidden)
}

fn check_role_assignment(current_user: &AppUser, role: Option<&str>) -> Result<()> {
    match role.unwrap_or("USER") {
        "USER" => Ok(()),
        "ADMIN" => {
            if current_user.role == "SUPER_ADMIN" {
                Ok(())
            } else {
                Err(crate::error::AppError::Forbidden)
            }
        }
        "SUPER_ADMIN" => Err(crate::error::AppError::Forbidden),
        _ => Err(crate::error::AppError::Validation("无效角色".into())),
    }
}

/// 按当前用户角色过滤可见用户列表（Low9）。
/// ADMIN 只能看到 USER 角色与自己的账号，防止枚举同级/上级账号，
/// 同时保留管理员自改密码（M14）的入口；SUPER_ADMIN 返回全部。
fn filter_visible_users(current_user_id: &str, role: &str, users: Vec<AppUser>) -> Vec<AppUser> {
    if role == "ADMIN" {
        users
            .into_iter()
            .filter(|u| u.role == "USER" || u.id == current_user_id)
            .collect()
    } else {
        users
    }
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    axum::extract::Query(_params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>> {
    // Low9：ADMIN 只能看到 USER 角色与自身（保留自改密码入口）；SUPER_ADMIN 返回全部。
    // 防止低权管理员枚举同级/上级账号的角色、启停状态与账户统计。
    // 密码哈希已 skip_serializing，不会经列表泄露。
    let users = filter_visible_users(
        &current_user.id,
        &current_user.role,
        db::list_users(&state.db).await?,
    );
    let stats = db::list_user_account_stats(&state.db).await?;
    let data: Vec<Value> = users
        .into_iter()
        .map(|user| {
            let user_stats = stats.get(&user.id);
            json!({
                "id": user.id,
                "username": user.username,
                "role": user.role,
                "enabled": user.enabled,
                "note": user.note,
                "createdAt": user.created_at,
                "updatedAt": user.updated_at,
                "accountCount": user_stats.map(|s| s.account_count).unwrap_or(0),
                "enabledAccountCount": user_stats.map(|s| s.enabled_account_count).unwrap_or(0),
                "failedAccountCount": user_stats.map(|s| s.failed_account_count).unwrap_or(0),
                "lastRunAt": user_stats.and_then(|s| s.last_run_at.as_ref()).cloned(),
            })
        })
        .collect();
    Ok(crate::routes::data(data))
}

pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let user = db::find_user_by_id(&state.db, &id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    if user.id != current_user.id {
        check_admin_permission(&current_user, &user)?;
    }
    Ok(crate::routes::data(user))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<Value>> {
    check_role_assignment(&current_user, payload.role.as_deref())?;

    if payload.username.trim().is_empty() {
        return Err(crate::error::AppError::Validation("用户名不能为空".into()));
    }
    if payload.password.len() < 8 {
        return Err(crate::error::AppError::Validation(
            "密码至少需要 8 位".into(),
        ));
    }

    if db::find_user_by_username(&state.db, &payload.username)
        .await?
        .is_some()
    {
        return Err(crate::error::AppError::Conflict("用户名已存在".into()));
    }

    let password_hash = hash_password(&payload.password)?;
    let user = db::create_user(
        &state.db,
        &payload.username,
        &password_hash,
        payload.role.as_deref().unwrap_or("USER"),
        payload.enabled.unwrap_or(true),
        payload.note.as_deref(),
    )
    .await?;
    Ok(crate::routes::data(user))
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<Value>> {
    let existing = db::find_user_by_id(&state.db, &id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;

    let is_self = existing.id == current_user.id;
    if is_self {
        // M14：允许管理员修改自己的资料/密码（原 check_admin_permission 禁止同级管理，
        // 导致管理员改不了自己）。自改不允许变更角色，防止升权或降级越界。
        if let Some(role) = payload.role.as_deref() {
            if role != existing.role {
                return Err(crate::error::AppError::Forbidden);
            }
        }
    } else {
        check_admin_permission(&current_user, &existing)?;
        check_role_assignment(&current_user, payload.role.as_deref())?;
    }

    if existing.role == "SUPER_ADMIN" && payload.role.as_deref().is_some_and(|r| r != "SUPER_ADMIN")
    {
        return Err(crate::error::AppError::Forbidden);
    }

    // M13：改用户名前预查唯一性，避免撞 DB UNIQUE 约束直接 500
    if let Some(new_username) = payload.username.as_deref() {
        if new_username != existing.username {
            if new_username.trim().is_empty() {
                return Err(crate::error::AppError::Validation("用户名不能为空".into()));
            }
            if db::find_user_by_username(&state.db, new_username)
                .await?
                .is_some()
            {
                return Err(crate::error::AppError::Conflict("用户名已存在".into()));
            }
        }
    }

    let password_hash = if let Some(pwd) = &payload.password {
        if pwd.len() < 8 {
            return Err(crate::error::AppError::Validation(
                "密码至少需要 8 位".into(),
            ));
        }
        Some(hash_password(pwd)?)
    } else {
        None
    };

    db::update_user(
        &state.db,
        &id,
        payload.username.as_deref(),
        password_hash.as_deref(),
        payload.role.as_deref(),
        payload.enabled,
        payload.note.as_deref(),
    )
    .await?;

    // Low10：改密后删除该用户全部会话，旧 Cookie 立即失效
    if password_hash.is_some() {
        db::delete_sessions_for_user(&state.db, &id).await?;
    }

    let user = db::find_user_by_id(&state.db, &id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(crate::routes::data(user))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let user = db::find_user_by_id(&state.db, &id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;

    check_admin_permission(&current_user, &user)?;

    if user.role == "SUPER_ADMIN" {
        return Err(crate::error::AppError::Forbidden);
    }

    db::delete_user(&state.db, &id).await?;
    Ok(crate::routes::data(json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::filter_visible_users;
    use crate::models::AppUser;
    use chrono::Utc;

    fn user(id: &str, role: &str) -> AppUser {
        let now = Utc::now();
        AppUser {
            id: id.to_string(),
            username: format!("{id}-name"),
            password_hash: String::new(),
            role: role.to_string(),
            enabled: true,
            note: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn admin_sees_user_role_and_self_but_not_peer_admins() {
        let users = vec![
            user("u1", "USER"),
            user("me", "ADMIN"),
            user("other-admin", "ADMIN"),
            user("s1", "SUPER_ADMIN"),
        ];

        let visible = filter_visible_users("me", "ADMIN", users.clone());
        assert_eq!(visible.len(), 2);
        let ids: Vec<&str> = visible.iter().map(|u| u.id.as_str()).collect();
        assert!(ids.contains(&"u1"));
        assert!(ids.contains(&"me"));
        assert!(!ids.contains(&"other-admin"));
        assert!(!ids.contains(&"s1"));

        let all = filter_visible_users("root", "SUPER_ADMIN", users);
        assert_eq!(all.len(), 4);
    }
}
