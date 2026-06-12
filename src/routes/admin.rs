use axum::{
    extract::{Path, State},
    Extension,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{
    models::{AppUser, UpdateUserRequest},
    db,
    crypto::hash_password,
    AppState,
};

type Result<T> = std::result::Result<T, crate::error::AppError>;

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
        _ => Err(crate::error::AppError::Validation("Invalid role".into())),
    }
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
) -> Result<Json<Vec<AppUser>>> {
    let mut users = db::list_users(&state.db).await?;
    if current_user.role == "ADMIN" {
        users.retain(|user| user.role == "USER");
    }
    Ok(Json(users))
}

pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    Path(id): Path<String>,
) -> Result<Json<AppUser>> {
    let user = db::find_user_by_id(&state.db, &id).await?
        .ok_or(crate::error::AppError::NotFound)?;
    if user.id != current_user.id {
        check_admin_permission(&current_user, &user)?;
    }
    Ok(Json(user))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<AppUser>> {
    check_role_assignment(&current_user, payload.role.as_deref())?;

    if payload.username.trim().is_empty() {
        return Err(crate::error::AppError::Validation("Username cannot be empty".into()));
    }
    if payload.password.len() < 8 {
        return Err(crate::error::AppError::Validation("Password must be at least 8 characters".into()));
    }

    if db::find_user_by_username(&state.db, &payload.username).await?.is_some() {
        return Err(crate::error::AppError::Validation("Username already exists".into()));
    }

    let password_hash = hash_password(&payload.password)?;
    let user = db::create_user(
        &state.db,
        &payload.username,
        &password_hash,
        payload.role.as_deref().unwrap_or("USER"),
        payload.enabled.unwrap_or(true),
        payload.note.as_deref(),
    ).await?;
    Ok(Json(user))
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<AppUser>> {
    let existing = db::find_user_by_id(&state.db, &id).await?
        .ok_or(crate::error::AppError::NotFound)?;
    
    check_admin_permission(&current_user, &existing)?;
    check_role_assignment(&current_user, payload.role.as_deref())?;
    
    if existing.role == "SUPER_ADMIN" && payload.role.as_deref() != Some("SUPER_ADMIN") {
        return Err(crate::error::AppError::Forbidden);
    }
    
    let password_hash = if let Some(pwd) = &payload.password {
        if pwd.len() < 8 {
            return Err(crate::error::AppError::Validation("Password must be at least 8 characters".into()));
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
    ).await?;
    
    let user = db::find_user_by_id(&state.db, &id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(user))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(current_user): Extension<AppUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let user = db::find_user_by_id(&state.db, &id).await?
        .ok_or(crate::error::AppError::NotFound)?;
    
    check_admin_permission(&current_user, &user)?;
    
    if user.role == "SUPER_ADMIN" {
        return Err(crate::error::AppError::Forbidden);
    }
    
    db::delete_user(&state.db, &id).await?;
    Ok(Json(json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub note: Option<String>,
}
