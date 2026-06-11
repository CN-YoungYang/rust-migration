use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
    http::StatusCode,
};
use std::sync::Arc;
use crate::{AppState, db, error::AppError};

// Simple session store (in production, use Redis or similar)
use std::collections::HashMap;
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref SESSIONS: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

pub fn create_session(user_id: &str) -> String {
    let token = uuid::Uuid::new_v4().to_string();
    let mut sessions = SESSIONS.lock().unwrap();
    sessions.insert(token.clone(), user_id.to_string());
    token
}

pub fn get_user_from_session(token: &str) -> Option<String> {
    let sessions = SESSIONS.lock().unwrap();
    sessions.get(token).cloned()
}

pub fn remove_session(token: &str) {
    let mut sessions = SESSIONS.lock().unwrap();
    sessions.remove(token);
}

// Auth middleware
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());
    
    if let Some(auth_value) = auth_header {
        if let Some(token) = auth_value.strip_prefix("Bearer ") {
            if let Some(user_id) = get_user_from_session(token) {
                if let Ok(Some(user)) = db::find_user_by_id(&state.db, &user_id).await {
                    if user.enabled {
                        request.extensions_mut().insert(user);
                        return Ok(next.run(request).await);
                    }
                }
            }
        }
    }
    
    Err(StatusCode::UNAUTHORIZED)
}

// Admin-only middleware
pub async fn admin_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let user = request.extensions().get::<crate::models::AppUser>().cloned();
    
    match user {
        Some(u) if u.role == "ADMIN" => Ok(next.run(request).await),
        _ => Err(StatusCode::FORBIDDEN),
    }
}
