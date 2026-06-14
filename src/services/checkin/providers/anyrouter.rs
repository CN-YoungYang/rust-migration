use crate::error::Result;
use super::super::http_client;

const DEFAULT_CHECKIN_PATH: &str = "/api/user/sign_in";
const ANYROUTER_CHALLENGE_MESSAGE: &str = "签到失败：yrouter 返回反爬挑战页，当前 Cookie 可能已失效。请在浏览器重新登录 yrouter，复制最新 Cookie 后更新账号。";
const ACW_SC_V2_KEY: &str = "3000176000856006061501533003690027800375";
const ACW_SC_V2_INDEXES: [usize; 40] = [
    0xf, 0x23, 0x1d, 0x18, 0x21, 0x10, 0x1, 0x26, 0xa, 0x9, 0x13, 0x1f, 0x28, 0x1b, 0x16, 0x17,
    0x19, 0xd, 0x6, 0xb, 0x27, 0x12, 0x14, 0x8, 0xe, 0x15, 0x20, 0x1a, 0x2, 0x1e, 0x7, 0x4, 0x11,
    0x5, 0x3, 0x1c, 0x22, 0x25, 0xc, 0x24,
];

fn join_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn is_already_checked_message(message: &str) -> bool {
    let normalized = message.trim().to_lowercase();
    if normalized.is_empty() {
        return true;
    }
    ["已签", "已经签到", "今天已经签到", "already"]
        .iter()
        .any(|text| normalized.contains(text))
}

fn is_success_message(message: &str) -> bool {
    message.contains("签到成功")
}

fn is_challenge_page(response_text: &str, content_type: Option<&str>) -> bool {
    let ct = content_type.unwrap_or("").to_lowercase();
    let trimmed = response_text.trim().to_lowercase();
    (ct.contains("text/html") || trimmed.starts_with("<html"))
        && response_text.contains("acw_sc__v2")
        && response_text.contains("var arg1")
}

fn extract_arg1(text: &str) -> Option<String> {
    let mut rest = text;
    while let Some(idx) = rest.find("arg1") {
        let after = &rest[idx + 4..];
        let trimmed = after.trim_start();
        if !trimmed.starts_with('=') {
            rest = &rest[idx + 4..];
            continue;
        }
        let after_eq = trimmed[1..].trim_start();
        let quote = after_eq.chars().next()?;
        if quote != '\'' && quote != '"' {
            rest = &rest[idx + 4..];
            continue;
        }
        let inner = &after_eq[1..];
        let end = inner.find(quote)?;
        let hex = &inner[..end];
        if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(hex.to_string());
        }
        rest = &rest[idx + 4..];
    }
    None
}

fn solve_acw_sc_v2(response_text: &str) -> Option<String> {
    let arg1 = extract_arg1(response_text)?;
    if arg1.len() != ACW_SC_V2_INDEXES.len() {
        return None;
    }

    let mut reordered = vec![' '; arg1.len()];
    for (i, ch) in arg1.chars().enumerate() {
        let target = i + 1;
        if let Some(pos) = ACW_SC_V2_INDEXES.iter().position(|&v| v == target) {
            reordered[pos] = ch;
        }
    }

    let hex: String = reordered.iter().collect();
    if hex.len() != arg1.len() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let hex_bytes = hex.as_bytes();
    let key_bytes = ACW_SC_V2_KEY.as_bytes();
    let mut value = String::new();
    let mut i = 0;
    while i + 2 <= hex_bytes.len() && i + 2 <= key_bytes.len() {
        let h = u8::from_str_radix(std::str::from_utf8(&hex_bytes[i..i + 2]).ok()?, 16).ok()?;
        let k = u8::from_str_radix(std::str::from_utf8(&key_bytes[i..i + 2]).ok()?, 16).ok()?;
        value.push_str(&format!("{:02x}", h ^ k));
        i += 2;
    }

    Some(value)
}

fn merge_cookie(cookie: Option<&str>, name: &str, value: &str) -> String {
    let prefix = format!("{}=", name.to_lowercase());
    let mut segments: Vec<String> = cookie
        .unwrap_or("")
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|s| !s.to_lowercase().starts_with(&prefix))
        .collect();
    segments.push(format!("{}={}", name, value));
    segments.join("; ")
}

fn read_message(payload: Option<&serde_json::Value>) -> String {
    payload
        .and_then(|p| {
            p.get("message")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| p.get("error").and_then(|v| v.as_str()))
        })
        .unwrap_or("")
        .to_string()
}

async fn post_checkin(
    client: &reqwest::Client,
    url: &str,
    user_id: Option<&str>,
    cookie: Option<&str>,
) -> Result<(reqwest::StatusCode, String, Option<String>)> {
    let mut req = client
        .post(url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Pragma", "no-cache")
        .header("X-Requested-With", "XMLHttpRequest")
        .body("{}".to_string());

    if let Some(uid) = user_id {
        req = req.header("User-id", uid);
    }
    if let Some(c) = cookie {
        req = req.header("Cookie", c);
    }

    let response = req.send().await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let text = response.text().await?;

    Ok((status, text, content_type))
}

pub async fn checkin(
    base_url: &str,
    user_id: Option<&str>,
    cookie: Option<&str>,
    custom_url: Option<&str>,
) -> Result<(String, String, Option<String>)> {
    let endpoint = custom_url.unwrap_or(DEFAULT_CHECKIN_PATH);
    let url = join_url(base_url, endpoint);
    let client = http_client();

    let (mut status, mut text, mut content_type) =
        post_checkin(&client, &url, user_id, cookie).await?;

    if is_challenge_page(&text, content_type.as_deref()) {
        if let Some(acw_sc_v2) = solve_acw_sc_v2(&text) {
            let merged = merge_cookie(cookie, "acw_sc__v2", &acw_sc_v2);
            let (s, t, ct) = post_checkin(&client, &url, user_id, Some(&merged)).await?;
            status = s;
            text = t;
            content_type = ct;
        }
    }

    if is_challenge_page(&text, content_type.as_deref()) {
        return Ok((
            "failed".to_string(),
            ANYROUTER_CHALLENGE_MESSAGE.to_string(),
            Some(text),
        ));
    }

    let payload: Option<serde_json::Value> = serde_json::from_str(&text).ok();
    let response_message = read_message(payload.as_ref());

    if !status.is_success() {
        if is_already_checked_message(&response_message) {
            return Ok((
                "already_checked".to_string(),
                if response_message.is_empty() {
                    "今日已签".to_string()
                } else {
                    response_message
                },
                Some(text),
            ));
        }
        return Ok((
            "failed".to_string(),
            if response_message.is_empty() {
                format!("签到请求失败：HTTP {}", status)
            } else {
                response_message
            },
            Some(text),
        ));
    }

    let success = payload
        .as_ref()
        .and_then(|p| p.get("success"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if success && is_success_message(&response_message) {
        return Ok((
            "success".to_string(),
            if response_message.is_empty() {
                "签到成功".to_string()
            } else {
                response_message
            },
            Some(text),
        ));
    }

    if success && is_already_checked_message(&response_message) {
        return Ok((
            "already_checked".to_string(),
            if response_message.is_empty() {
                "今日已签".to_string()
            } else {
                response_message
            },
            Some(text),
        ));
    }

    if success {
        return Ok((
            "success".to_string(),
            if response_message.is_empty() {
                "签到成功".to_string()
            } else {
                response_message
            },
            Some(text),
        ));
    }

    if is_already_checked_message(&response_message) {
        return Ok((
            "already_checked".to_string(),
            if response_message.is_empty() {
                "今日已签".to_string()
            } else {
                response_message
            },
            Some(text),
        ));
    }

    Ok((
        "failed".to_string(),
        if response_message.is_empty() {
            "签到失败：站点未返回成功状态".to_string()
        } else {
            response_message
        },
        Some(text),
    ))
}
