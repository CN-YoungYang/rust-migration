use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
    http::StatusCode,
};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use crate::{AppState, db};

#[derive(Clone)]
struct SessionEntry {
    user_id: String,
    expires_at: SystemTime,
}

static SESSIONS: LazyLock<Mutex<HashMap<String, SessionEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const MAX_SESSIONS: usize = 1000;

static SESSION_TTL: OnceLock<Duration> = OnceLock::new();

fn session_ttl() -> Duration {
    *SESSION_TTL.get_or_init(|| {
        let hours = std::env::var("SESSION_TTL_HOURS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|hours| *hours > 0)
            .unwrap_or(24);
        Duration::from_secs(hours * 60 * 60)
    })
}

pub fn create_session(user_id: &str) -> String {
    let token = uuid::Uuid::new_v4().to_string();
    let expires_at = SystemTime::now() + session_ttl();
    let mut sessions = SESSIONS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    if sessions.len() % 10 == 0 {
        cleanup_expired_sessions(&mut sessions);
    }

    sessions.insert(token.clone(), SessionEntry {
        user_id: user_id.to_string(),
        expires_at,
    });
    token
}

pub fn get_user_from_session(token: &str) -> Option<String> {
    let mut sessions = SESSIONS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = sessions.get(token)?.clone();
    if SystemTime::now() >= entry.expires_at {
        sessions.remove(token);
        return None;
    }
    Some(entry.user_id)
}

pub fn remove_session(token: &str) {
    let mut sessions = SESSIONS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    sessions.remove(token);
}

fn cleanup_expired_sessions(sessions: &mut HashMap<String, SessionEntry>) {
    let now = SystemTime::now();
    sessions.retain(|_, entry| now < entry.expires_at);

    // Hard cap: evict oldest entries if still over limit
    if sessions.len() > MAX_SESSIONS {
        let mut entries: Vec<_> = sessions.iter().collect();
        entries.sort_by_key(|(_, e)| e.expires_at);
        let to_remove = sessions.len() - MAX_SESSIONS;
        let tokens_to_remove: Vec<_> = entries.into_iter().take(to_remove).map(|(k, _)| k.clone()).collect();
        for token in tokens_to_remove {
            sessions.remove(&token);
        }
    }
}

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

pub async fn admin_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let user = request.extensions().get::<crate::models::AppUser>().cloned();

    match user {
        Some(u) if u.role == "ADMIN" || u.role == "SUPER_ADMIN" => Ok(next.run(request).await),
        _ => Err(StatusCode::FORBIDDEN),
    }
}
