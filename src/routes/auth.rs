use axum::{
    extract::State,
    Json,
};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant;
use crate::{
    AppState,
    models::{LoginRequest, LoginResponse, AppUser},
    error::Result,
    crypto::verify_password,
    db,
    auth_middleware::{create_session, remove_session},
};

const DUMMY_BCRYPT_HASH: &str = "$2b$10$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
use serde_json::{json, Value};
use std::sync::{LazyLock, Mutex};

const MAX_LOGIN_ATTEMPTS: u8 = 5;
const LOGIN_LOCKOUT_SECS: u64 = 300; // 5 minutes

#[derive(Clone)]
struct LoginAttempt {
    count: u8,
    first_attempt: Instant,
}

static LOGIN_ATTEMPTS: LazyLock<Mutex<HashMap<String, LoginAttempt>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn check_login_rate(username: &str) -> Result<()> {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());

    // Periodic cleanup: evict expired entries
    if attempts.len() > 100 {
        attempts.retain(|_, e| e.first_attempt.elapsed().as_secs() < LOGIN_LOCKOUT_SECS);
    }

    if let Some(entry) = attempts.get(username) {
        if entry.count >= MAX_LOGIN_ATTEMPTS {
            if entry.first_attempt.elapsed().as_secs() < LOGIN_LOCKOUT_SECS {
                return Err(crate::error::AppError::Validation(
                    format!("登录尝试过于频繁，请 {} 秒后重试",
                        LOGIN_LOCKOUT_SECS - entry.first_attempt.elapsed().as_secs())
                ));
            }
            attempts.remove(username);
        }
    }
    Ok(())
}

fn record_failed_login(username: &str) {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());
    let entry = attempts.entry(username.to_string()).or_insert(LoginAttempt {
        count: 0,
        first_attempt: Instant::now(),
    });
    entry.count += 1;
}

fn clear_login_attempts(username: &str) {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());
    attempts.remove(username);
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    check_login_rate(&payload.username)?;

    let user = db::find_user_by_username(&state.db, &payload.username).await?;

    let (user_id, hash) = match &user {
        Some(u) if u.enabled => (Some(u.id.clone()), u.password_hash.clone()),
        _ => (None, DUMMY_BCRYPT_HASH.to_string()),
    };

    let valid = verify_password(&payload.password, &hash).unwrap_or(false);

    match (user_id, valid) {
        (Some(uid), true) => {
            clear_login_attempts(&payload.username);
            let token = create_session(&uid);
            // 复用第一次查询结果，避免重复 DB 查询
            let user = user.unwrap();
            Ok(Json(LoginResponse { token, user }))
        }
        _ => {
            record_failed_login(&payload.username);
            Err(crate::error::AppError::Unauthorized)
        }
    }
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

pub async fn me(
    request: axum::http::Request<axum::body::Body>,
) -> Result<Json<Value>> {
    let user = request.extensions().get::<AppUser>().cloned();
    Ok(Json(json!({ "user": user })))
}
