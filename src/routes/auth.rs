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
/// IP 取“真实客户端 IP”：直连对端是受信任代理时才采用 X-Forwarded-For，
/// 否则用连接对端 IP，防止伪造。
fn rate_key(ip: std::net::IpAddr, username: &str) -> String {
    format!("{}|{}", ip, username)
}

/// 解析代理白名单：支持精确 IP 与 CIDR（如 `127.0.0.1`、`172.16.0.0/12`、`::1`）。
/// 返回 `(网段基址, 前缀长度)`；前缀为 `None` 表示整段精确匹配。
fn parse_proxy_entries(spec: &str) -> Vec<(std::net::IpAddr, Option<u8>)> {
    spec.split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            if let Some((ip_str, prefix_str)) = part.split_once('/') {
                let ip: std::net::IpAddr = ip_str.trim().parse().ok()?;
                let prefix: u8 = prefix_str.trim().parse().ok()?;
                let max = if ip.is_ipv4() { 32 } else { 128 };
                Some((ip, Some(prefix.min(max))))
            } else {
                let ip: std::net::IpAddr = part.parse().ok()?;
                Some((ip, None))
            }
        })
        .collect()
}

/// IP 是否命中白名单条目（IP 或 CIDR）。
fn ip_matches(ip: std::net::IpAddr, entry: (std::net::IpAddr, Option<u8>)) -> bool {
    let (base, prefix) = entry;
    use std::net::IpAddr;
    match (ip, base) {
        (IpAddr::V4(a), IpAddr::V4(b)) => {
            let p = prefix.unwrap_or(32) as u32;
            if p == 0 {
                return true;
            }
            let mask = u32::MAX << (32 - p);
            (u32::from(a) & mask) == (u32::from(b) & mask)
        }
        (IpAddr::V6(a), IpAddr::V6(b)) => {
            let p = prefix.unwrap_or(128) as u32;
            if p == 0 {
                return true;
            }
            let mask = if p >= 128 {
                u128::MAX
            } else {
                u128::MAX << (128 - p)
            };
            (u128::from(a) & mask) == (u128::from(b) & mask)
        }
        _ => false,
    }
}

/// 对端 IP 是否可信代理：loopback 恒可信；其余须命中 `TRUSTED_PROXY_IPS`。
fn trusted_proxy(peer: std::net::IpAddr) -> bool {
    peer.is_loopback()
        || parse_proxy_entries(&std::env::var("TRUSTED_PROXY_IPS").unwrap_or_default())
            .into_iter()
            .any(|entry| ip_matches(peer, entry))
}

/// 解析真实客户端 IP 用于限流：
/// 直连对端是受信任代理时，采用 `X-Forwarded-For` 最左一项（原始客户端，
/// nginx/负载均衡会覆盖该头，取最左即原始来源）；否则用连接对端 IP。
/// 不信任来源的 X-Forwarded-For 一律忽略，防止直连客户端伪造换桶绕过限流。
fn effective_client_ip(peer: std::net::IpAddr, headers: &HeaderMap) -> std::net::IpAddr {
    if trusted_proxy(peer) {
        if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first) = xff
                .split(',')
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if let Ok(ip) = first.parse() {
                    return ip;
                }
            }
        }
    }
    peer
}

/// 逐出过期条目；仍超上限时仅逐出“未锁定”（count < MAX）的最旧条目。
/// 剩余条目若全在锁定中，则不再逐出——避免把锁定中的受害者提前解锁（Low13），
/// 由 `record_failed_login` 对“新 key”跳过记录以保证内存有界。
fn prune_login_attempts(attempts: &mut HashMap<String, LoginAttempt>) {
    attempts.retain(|_, e| e.first_attempt.elapsed().as_secs() < LOGIN_LOCKOUT_SECS);
    if attempts.len() <= MAX_RATE_ENTRIES {
        return;
    }
    let over = attempts.len() - MAX_RATE_ENTRIES;
    let mut unlocked: Vec<(String, Instant)> = attempts
        .iter()
        .filter(|(_, e)| e.count < MAX_LOGIN_ATTEMPTS)
        .map(|(k, e)| (k.clone(), e.first_attempt))
        .collect();
    unlocked.sort_by_key(|(_, t)| *t);
    for (k, _) in unlocked.into_iter().take(over) {
        attempts.remove(&k);
    }
}

fn check_login_rate(key: &str) -> Result<()> {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());
    prune_login_attempts(&mut attempts);

    if let Some(entry) = attempts.get(key) {
        if entry.count >= MAX_LOGIN_ATTEMPTS {
            let remaining =
                LOGIN_LOCKOUT_SECS.saturating_sub(entry.first_attempt.elapsed().as_secs());
            if remaining > 0 {
                // 锁定返回专用消息与剩余秒数，供前端展示倒计时；状态码仍非 2xx，
                // 且对不存在用户名同样会触发锁定，不泄露用户名是否存在（M4）。
                return Err(AppError::RateLimited(remaining));
            }
            attempts.remove(key);
        }
    }
    Ok(())
}

fn record_failed_login(key: &str) {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());
    prune_login_attempts(&mut attempts);
    if let Some(entry) = attempts.get_mut(key) {
        entry.count += 1;
        return;
    }
    // 表已满且剩余均为锁定条目时不记录新 key：不挤占内存，
    // 也避免把仍在锁定期的受害者条目驱逐导致提前解锁（Low13）。
    if attempts.len() >= MAX_RATE_ENTRIES {
        return;
    }
    attempts.insert(
        key.to_string(),
        LoginAttempt {
            count: 1,
            first_attempt: Instant::now(),
        },
    );
}

fn clear_login_attempts(key: &str) {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap_or_else(|p| p.into_inner());
    attempts.remove(key);
}

/// 提取 URL/裸 Host 字符串的 hostname（小写），忽略 scheme、端口、大小写。
/// 如 `http://Example.com:8080` 与 `example.com` 均返回 `example.com`。
/// 解析失败返回 None（调用方视为非同源）。
fn url_host(input: &str) -> Option<String> {
    let normalized = if input.contains("://") {
        input.to_string()
    } else {
        format!("https://{input}")
    };
    reqwest::Url::parse(&normalized)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
}

/// M3：登录接口校验 Origin，拦截跨站请求伪造（CSRF）。
/// 登录页由同源前端提供，浏览器对 POST 会带 Origin；Origin 缺失（curl/同源导航）
/// 放行，出现时须为同源或 CORS 白名单来源，否则拒绝。
/// 同源判定只比较 hostname（忽略 scheme/端口/大小写）：反代改写 Host（去掉端口、
/// 换成内部名）时不误拒同源登录；代理携带的 X-Forwarded-Host（原始 Host）也纳入比较。
fn validate_login_origin(headers: &HeaderMap) -> Result<()> {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };

    let origin_host = url_host(origin);
    let host_matches = if let Some(origin_host) = origin_host {
        let mut hosts: Vec<&str> = Vec::new();
        if let Some(h) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) {
            hosts.push(h);
        }
        if let Some(h) = headers
            .get("x-forwarded-host")
            .and_then(|v| v.to_str().ok())
        {
            hosts.push(h);
        }
        hosts
            .into_iter()
            .filter_map(url_host)
            .any(|hh| hh == origin_host)
    } else {
        false
    };
    if host_matches {
        return Ok(());
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

    // M4：取真实客户端 IP。反代部署（nginx/负载均衡）下直连对端是代理，
    // 用 X-Forwarded-For 还原客户端 IP；直连时用 ConnectInfo 对端 IP（防伪造）。
    let key = rate_key(effective_client_ip(addr.ip(), &headers), &payload.username);
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
    fn client_ip_trusts_xff_only_from_trusted_proxy() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));

        // loopback 对端默认受信任（本机反代场景）→ 采用 XFF
        let direct = "127.0.0.1".parse().unwrap();
        assert_eq!(effective_client_ip(direct, &h).to_string(), "203.0.113.7");

        // 公网直连对端不受信任 → 忽略 XFF（防伪造）
        let public_peer = "198.51.100.4".parse().unwrap();
        assert_eq!(
            effective_client_ip(public_peer, &h).to_string(),
            "198.51.100.4"
        );

        // 配置 TRUSTED_PROXY_IPS（支持 CIDR）后，命中网段的代理对端采用 XFF
        std::env::set_var("TRUSTED_PROXY_IPS", "172.16.0.0/12, 10.0.0.7");
        let gateway = "172.17.0.1".parse().unwrap();
        assert_eq!(effective_client_ip(gateway, &h).to_string(), "203.0.113.7");
        let exact = "10.0.0.7".parse().unwrap();
        assert_eq!(effective_client_ip(exact, &h).to_string(), "203.0.113.7");
        // 未命中白名单的私网对端不受信任
        let other_private = "192.168.1.9".parse().unwrap();
        assert_eq!(
            effective_client_ip(other_private, &h).to_string(),
            "192.168.1.9"
        );
        std::env::remove_var("TRUSTED_PROXY_IPS");
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

    #[test]
    fn login_origin_tolerates_port_scheme_and_case_differences() {
        // 反代改写 Host 去掉端口后，hostname 相同仍视为同源
        let mut h = HeaderMap::new();
        h.insert(header::HOST, HeaderValue::from_static("example.com"));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://example.com:8080"),
        );
        assert!(validate_login_origin(&h).is_ok());

        // scheme/大小写差异不影响 hostname 比较
        let mut h2 = HeaderMap::new();
        h2.insert(header::HOST, HeaderValue::from_static("example.com"));
        h2.insert(
            header::ORIGIN,
            HeaderValue::from_static("HTTPS://EXAMPLE.COM"),
        );
        assert!(validate_login_origin(&h2).is_ok());

        // 不同 hostname 依旧拒绝
        let mut h3 = HeaderMap::new();
        h3.insert(header::HOST, HeaderValue::from_static("example.com"));
        h3.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://evil.example.com"),
        );
        assert!(matches!(
            validate_login_origin(&h3),
            Err(AppError::Forbidden)
        ));
    }

    #[test]
    fn login_origin_uses_x_forwarded_host_when_host_is_rewritten() {
        // Host 被改写为内部名时，代理携带的 X-Forwarded-Host 还原原始 Host
        let mut h = HeaderMap::new();
        h.insert(header::HOST, HeaderValue::from_static("internal-svc:8080"));
        h.insert("x-forwarded-host", HeaderValue::from_static("example.com"));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://example.com"),
        );
        assert!(validate_login_origin(&h).is_ok());

        // 无 X-Forwarded-Host 时，改写的 Host 无法匹配公共 Origin → 拒绝（需白名单兜底）
        let mut h2 = HeaderMap::new();
        h2.insert(header::HOST, HeaderValue::from_static("internal-svc:8080"));
        h2.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://example.com"),
        );
        assert!(matches!(
            validate_login_origin(&h2),
            Err(AppError::Forbidden)
        ));
    }
}
