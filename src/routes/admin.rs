use axum::{
    extract::{State, Path},
    Json,
};
use std::sync::Arc;
use serde::{Deserialize};
use serde_json::{json, Value};
use crate::{
    AppState,
    models::AppUser,
    error::Result,
    crypto::hash_password,
    db,
};

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub note: Option<String>,
}

pub async fn list_users(State(state): State<Arc<AppState>>) -> Result<Json<Vec<AppUser>>> {
    let users = db::list_users(&state.db).await?;
    Ok(Json(users))
}

pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AppUser>> {
    let user = db::find_user_by_id(&state.db, &id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    Ok(Json(user))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<AppUser>> {
    let password_hash = hash_password(&payload.password)?;
    let user = db::create_user(&state.db, &payload.username, &password_hash, &payload.role).await?;
    Ok(Json(user))
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<AppUser>> {
    let password_hash = if let Some(pwd) = &payload.password {
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
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    db::delete_user(&state.db, &id).await?;
    Ok(Json(json!({ "success": true })))
}
