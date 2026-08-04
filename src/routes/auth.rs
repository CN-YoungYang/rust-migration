use crate::{
    auth_middleware::{
        clear_csrf_cookie, clear_session_cookie, create_session, csrf_cookie, remove_session,
        session_cookie, session_token_from_headers,
    },
    crypto::verify_password,
    db,
    error::{AppError, Result},
    models::{AppUser, LoginRequest},
    AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

const DUMMY_BCRYPT_HASH: &str = "$2b$10$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
use serde_json::{json, Value};
use std::sync::{LazyLock, Mutex};

const MAX_LOGIN_ATTEMPTS: u8 = 5;
const LOGIN_LOCKOUT_SECS: u64 = 300; // 5 minutes
/// 登录限速表硬上限：防止唯一 IP|username 组合在锁定窗口内无界增长内存（Low 6）
const MAX_RATE_ENTRIES: usize = 5000;

#[derive(Clone)]
struct LoginAttempt {
    count: u8,
    first_attempt: Instant,
}

static LOGIN_ATTEMPTS: LazyLock<Mutex<HashMap<String, LoginAttempt>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 按 IP + username 组合限速，避免单一用户名被无认证攻击者持续锁定（M4）。
/// 取真实连接对端 IP（ConnectInfo），不用可伪造的 X-Forwarded-For。
fn rate_key(ip: std::net::IpAddr, username: &str) -> String {
    format!("{}|{}", ip, username)
}

/// 逐出过期条目；仍超过硬上限时按最早尝试时间逐出，保证内存有界。
fn prune_login_attempts(attempts: &mut HashMap<String, LoginAttempt>) {
    attempts.retain(|_, e| e.first_attempt.elapsed().as_secs() < LOGIN_LOCKOUT_SECS);
    if attempts.len() > MAX_RATE_ENTRIES {
        let mut oldest: Vec<(String, Instant)> = attempts
            .iter()
            .map(|(k, e)| (k.clone(), e.first_attempt))
            .collect();
        oldest.sort_by_key(|(_, t)| *t);
        for (k, _) in oldest.iter().take(attempts.len() - MAX_RATE_ENTRIES) {
            attempts.remove(k);
        }
    }
}

fn check_login_rate(key: &str) -> Result<()> {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());
    prune_login_attempts(&mut attempts);

    if let Some(entry) = attempts.get(key) {
        if entry.count >= MAX_LOGIN_ATTEMPTS {
            if entry.first_attempt.elapsed().as_secs() < LOGIN_LOCKOUT_SECS {
                // 锁定与密码错误统一返回 401，避免区分响应暴露用户名是否存在（M4）
                return Err(AppError::Unauthorized);
            }
            attempts.remove(key);
        }
    }
    Ok(())
}

fn record_failed_login(key: &str) {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());
    let entry = attempts.entry(key.to_string()).or_insert(LoginAttempt {
        count: 0,
        first_attempt: Instant::now(),
    });
    entry.count += 1;
}

fn clear_login_attempts(key: &str) {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());
    attempts.remove(key);
}

/// M3：登录接口校验 Origin，拦截跨站请求伪造（CSRF）。
/// 登录页由同源前端提供，浏览器对 POST 会带 Origin；Origin 缺失（curl/同源导航）
/// 放行，出现时须为同源（与 Host 一致）或 CORS 白名单来源，否则拒绝。
fn validate_login_origin(headers: &HeaderMap) -> Result<()> {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };

    if let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) {
        let origin_host = origin
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');
        if origin_host == host {
            return Ok(());
        }
    }

    let allowed = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .any(|o| !o.is_empty() && o == origin);
    if allowed {
        return Ok(());
    }

    tracing::warn!(origin = %origin, "拒绝跨源登录请求（Origin 不在白名单且非同源）");
    Err(AppError::Forbidden)
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Response> {
    validate_login_origin(&headers)?;

    let key = rate_key(addr.ip(), &payload.username);
    check_login_rate(&key)?;

    let user = db::find_user_by_username(&state.db, &payload.username).await?;

    let (user_id, hash) = match &user {
        Some(u) if u.enabled => (Some(u.id.clone()), u.password_hash.clone()),
        _ => (None, DUMMY_BCRYPT_HASH.to_string()),
    };

    let valid = verify_password(&payload.password, &hash).unwrap_or(false);

    match (user_id, valid) {
        (Some(uid), true) => {
            clear_login_attempts(&key);
            let session = create_session(&state.db, &uid).await?;
            // 复用第一次查询结果，避免重复 DB 查询
            let user = user.ok_or(AppError::Unauthorized)?;
            let mut response = crate::routes::data(json!({ "user": user })).into_response();
            response.headers_mut().append(
                header::SET_COOKIE,
                HeaderValue::from_str(&session_cookie(&session.id))
                    .map_err(|_| crate::error::AppError::Internal("生成会话 Cookie 失败".into()))?,
            );
            response.headers_mut().append(
                header::SET_COOKIE,
                HeaderValue::from_str(&csrf_cookie(&session.csrf_token))
                    .map_err(|_| crate::error::AppError::Internal("生成会话 Cookie 失败".into()))?,
            );
            Ok(response)
        }
        _ => {
            record_failed_login(&key);
            Err(crate::error::AppError::Unauthorized)
        }
    }
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    request: axum::http::Request<axum::body::Body>,
) -> Response {
    if let Some(token) = session_token_from_headers(request.headers()) {
        if let Err(e) = remove_session(&state.db, &token).await {
            tracing::warn!("Failed to remove session: {}", e);
        }
    }
    let mut response = (
        StatusCode::OK,
        crate::routes::data(json!({ "success": true })),
    )
        .into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&clear_session_cookie())
            .unwrap_or_else(|_| HeaderValue::from_static("session_id=; Max-Age=0; Path=/")),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&clear_csrf_cookie())
            .unwrap_or_else(|_| HeaderValue::from_static("csrf_token=; Max-Age=0; Path=/")),
    );
    response
}

pub async fn me(request: axum::http::Request<axum::body::Body>) -> Result<Json<Value>> {
    let user = request.extensions().get::<AppUser>().cloned();
    Ok(crate::routes::data(json!({ "user": user })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_key_scopes_by_ip_and_username() {
        let a = rate_key("1.2.3.4".parse().unwrap(), "admin");
        let b = rate_key("1.2.3.4".parse().unwrap(), "admin");
        let c = rate_key("1.2.3.5".parse().unwrap(), "admin");
        let d = rate_key("1.2.3.4".parse().unwrap(), "other");
        assert_eq!(a, b);
        assert_ne!(a, c); // 不同 IP 不共享桶
        assert_ne!(a, d); // 同 IP 不同用户名不共享桶
    }

    #[test]
    fn login_origin_allows_missing_and_same_origin() {
        // 无 Origin（curl / 同源导航）放行
        assert!(validate_login_origin(&HeaderMap::new()).is_ok());

        let mut h = HeaderMap::new();
        h.insert(header::HOST, HeaderValue::from_static("example.com:8080"));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://example.com:8080"),
        );
        assert!(validate_login_origin(&h).is_ok());
    }

    #[test]
    fn login_origin_rejects_cross_site_and_allows_whitelist() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, HeaderValue::from_static("example.com:8080"));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://evil.example"),
        );
        assert!(matches!(
            validate_login_origin(&h),
            Err(AppError::Forbidden)
        ));

        std::env::set_var("CORS_ALLOWED_ORIGINS", "https://front.example.com");
        let mut h2 = HeaderMap::new();
        h2.insert(header::HOST, HeaderValue::from_static("api.example.com"));
        h2.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://front.example.com"),
        );
        assert!(validate_login_origin(&h2).is_ok());
        std::env::remove_var("CORS_ALLOWED_ORIGINS");
    }
}
