use crate::{db, AppState};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use sqlx::SqlitePool;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const MAX_SESSIONS: usize = 1000;

static SESSION_TTL: OnceLock<Duration> = OnceLock::new();
static COOKIE_SECURE: OnceLock<bool> = OnceLock::new();

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

pub fn session_ttl_secs() -> u64 {
    session_ttl().as_secs()
}

fn cookie_secure() -> bool {
    *COOKIE_SECURE.get_or_init(|| {
        std::env::var("COOKIE_SECURE")
            .ok()
            .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
    })
}

fn secure_suffix() -> &'static str {
    if cookie_secure() {
        "; Secure"
    } else {
        ""
    }
}

pub async fn create_session(db: &SqlitePool, user_id: &str) -> crate::error::Result<db::DbSession> {
    db::create_session(db, user_id, session_ttl_secs(), MAX_SESSIONS).await
}

pub fn session_cookie(token: &str) -> String {
    format!(
        "session_id={}; Max-Age={}; Path=/; HttpOnly; SameSite=Lax{}",
        token,
        session_ttl_secs(),
        secure_suffix()
    )
}

pub fn clear_session_cookie() -> String {
    format!(
        "session_id=; Max-Age=0; Path=/; HttpOnly; SameSite=Lax{}",
        secure_suffix()
    )
}

pub fn csrf_cookie(token: &str) -> String {
    format!(
        "csrf_token={}; Max-Age={}; Path=/; SameSite=Lax{}",
        token,
        session_ttl_secs(),
        secure_suffix()
    )
}

pub fn clear_csrf_cookie() -> String {
    format!(
        "csrf_token=; Max-Age=0; Path=/; SameSite=Lax{}",
        secure_suffix()
    )
}

fn token_from_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|pair| pair.strip_prefix("session_id=").map(str::to_string))
        .filter(|token| !token.is_empty())
}

fn token_from_bearer(headers: &HeaderMap) -> Option<String> {
    let auth_value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())?;
    auth_value
        .strip_prefix("Bearer ")
        .map(str::to_string)
        .filter(|token| !token.is_empty())
}

pub fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    token_from_cookie(headers).or_else(|| token_from_bearer(headers))
}

fn is_csrf_required(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    )
}

fn validate_csrf(headers: &HeaderMap, entry: &db::DbSession) -> bool {
    headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .map(|token| token == entry.csrf_token)
        .unwrap_or(false)
}

pub async fn remove_session(db: &SqlitePool, token: &str) -> crate::error::Result<()> {
    db::delete_session(db, token).await
}

/// Start background session cleanup task (runs every 5 minutes)
pub fn start_session_cleanup_task(db: SqlitePool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
        loop {
            interval.tick().await;
            match db::cleanup_expired_sessions(&db).await {
                Ok(removed) if removed > 0 => {
                    tracing::debug!("Session cleanup: removed {} expired sessions", removed);
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("Session cleanup failed: {}", e),
            }
        }
    });
    tracing::info!("Session cleanup task started (every 5 minutes)");
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(token) = session_token_from_headers(request.headers()) {
        if let Ok(Some(entry)) = db::find_session(&state.db, &token).await {
            if is_csrf_required(request.method()) && !validate_csrf(request.headers(), &entry) {
                return Err(StatusCode::FORBIDDEN);
            }
            if let Ok(Some(user)) = db::find_user_by_id(&state.db, &entry.user_id).await {
                if user.enabled {
                    request.extensions_mut().insert(user);
                    return Ok(next.run(request).await);
                }
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

pub async fn admin_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let user = request
        .extensions()
        .get::<crate::models::AppUser>()
        .cloned();

    match user {
        Some(u) if u.role == "ADMIN" || u.role == "SUPER_ADMIN" => Ok(next.run(request).await),
        _ => Err(StatusCode::FORBIDDEN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn extracts_session_token_from_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; session_id=abc-123; other=value"),
        );

        assert_eq!(
            session_token_from_headers(&headers),
            Some("abc-123".to_string())
        );
    }

    #[test]
    fn falls_back_to_bearer_for_compatibility() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer legacy-token"),
        );

        assert_eq!(
            session_token_from_headers(&headers),
            Some("legacy-token".to_string())
        );
    }

    #[test]
    fn validates_csrf_header_against_session_entry() {
        let entry = db::DbSession {
            id: "s1".into(),
            user_id: "u1".into(),
            csrf_token: "csrf-123".into(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(60),
        };
        let mut headers = HeaderMap::new();
        headers.insert("x-csrf-token", HeaderValue::from_static("csrf-123"));

        assert!(validate_csrf(&headers, &entry));
    }
}
