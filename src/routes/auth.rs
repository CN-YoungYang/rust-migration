use axum::{
    extract::State,
    Json,
    http::Request,
};
use std::sync::Arc;
use crate::{
    AppState,
    models::{LoginRequest, LoginResponse, AppUser},
    error::Result,
    crypto::verify_password,
    db,
    auth_middleware::{create_session, remove_session, get_user_from_session},
};
use serde_json::{json, Value};

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    let user = db::find_user_by_username(&state.db, &payload.username)
        .await?
        .ok_or(crate::error::AppError::Unauthorized)?;
    
    if !user.enabled {
        return Err(crate::error::AppError::Unauthorized);
    }
    
    let valid = verify_password(&payload.password, &user.password_hash)?;
    if !valid {
        return Err(crate::error::AppError::Unauthorized);
    }
    
    let token = create_session(&user.id);
    
    Ok(Json(LoginResponse {
        token,
        user,
    }))
}

pub async fn logout(
    request: axum::http::Request<axum::body::Body>,
) -> Json<Value> {
    if let Some(auth_header) = request.headers().get("Authorization") {
        if let Ok(auth_value) = auth_header.to_str() {
            if let Some(token) = auth_value.strip_prefix("Bearer ") {
                remove_session(token);
            }
        }
    }
    Json(json!({ "success": true }))
}

pub async fn me<B>(
    request: Request<B>,
) -> Result<Json<Value>> {
    let user = request.extensions().get::<AppUser>().cloned();
    Ok(Json(json!({ "user": user })))
}
