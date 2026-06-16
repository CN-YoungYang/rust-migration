use crate::error::Result;
use super::super::{BrowserProfile, http_client};
use super::{CheckinResponse, classify_checkin_status, format_awarded_quota};

fn read_number(value: Option<&serde_json::Value>) -> Option<f64> {
    let v = value?;
    if let Some(n) = v.as_f64() {
        return Some(n);
    }
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            if let Ok(n) = trimmed.parse::<f64>() {
                return Some(n);
            }
        }
    }
    None
}

fn read_error_message(payload: Option<&serde_json::Value>) -> Option<String> {
    payload
        .and_then(|v| v.get("message").or_else(|| v.get("error")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 读取本次签到获得的额度（参考 Next.js readAwardedQuota）
/// 依次尝试 data.quota_awarded / data.quotaAwarded / data.quota
fn read_awarded_quota(data: Option<&serde_json::Value>) -> Option<f64> {
    let data = data?.as_object()?;
    let value = data
        .get("quota_awarded")
        .or_else(|| data.get("quotaAwarded"))
        .or_else(|| data.get("quota"))?;
    read_number(Some(value))
}

/// 读取 checked_in / checkedIn 布尔标志（参考 Next.js readCheckedInFlag）
fn read_checked_in_flag(data: Option<&serde_json::Value>) -> bool {
    let data = match data.and_then(|v| v.as_object()) {
        Some(d) => d,
        None => return false,
    };
    data.get("checked_in").and_then(|v| v.as_bool()).unwrap_or(false)
        || data.get("checkedIn").and_then(|v| v.as_bool()).unwrap_or(false)
}

pub async fn checkin(
    base_url: &str,
    user_id: Option<&str>,
    access_token: Option<&str>,
    cookie: Option<&str>,
    profile: &BrowserProfile,
) -> Result<(String, String, Option<String>)> {
    let url = format!("{}/api/user/checkin", base_url.trim_end_matches('/'));
    let client = http_client();

    let mut req = super::super::apply_browser_headers(
        client.post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Pragma", "no-cache")
            // 浏览器从同源 SPA 发起的 fetch 一致性头，缺失会被 WAF 扣分
            .header("Sec-Fetch-Site", "same-origin")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Dest", "empty")
            .header("Referer", base_url.trim_end_matches('/'))
            .body("{}"),
        profile,
    );

    if let Some(uid) = user_id {
        req = req
            .header("New-API-User", uid)
            .header("Veloera-User", uid)
            .header("X-Api-User", uid)
            .header("voapi-user", uid)
            .header("User-id", uid)
            .header("Rix-Api-User", uid)
            .header("neo-api-user", uid);
    }
    if let Some(token) = access_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    if let Some(c) = cookie {
        req = req.header("Cookie", c);
    }

    let response = req.send().await?;
    let status_code = response.status();
    let text = response.text().await?;

    if !status_code.is_success() {
        return Ok(("failed".to_string(), format!("签到请求失败：HTTP {}", status_code), Some(text)));
    }

    let parsed: CheckinResponse = serde_json::from_str(&text)
        .unwrap_or(CheckinResponse {
            success: false,
            message: Some("Failed to parse response".into()),
            data: None,
        });

    let message = parsed.message.unwrap_or_else(|| "No message".to_string());

    // 状态判定：已签关键词 > checked_in 标志 > success（与 Next.js 顺序对齐）
    let status = {
        let base = classify_checkin_status(parsed.success, &message);
        if base == "already_checked" || read_checked_in_flag(parsed.data.as_ref()) {
            "already_checked"
        } else {
            base
        }
    };

    // 解析本次获得额度，拼入消息（参考 Next.js runner.ts）
    let final_message = match read_awarded_quota(parsed.data.as_ref()) {
        Some(q) => format!("{}；本次获得额度：{}", message, format_awarded_quota(q)),
        None => message,
    };

    Ok((status.to_string(), final_message, Some(text)))
}

pub async fn fetch_balance(base_url: &str, user_id: Option<&str>, access_token: Option<&str>, cookie: Option<&str>, profile: &BrowserProfile) -> std::result::Result<f64, Box<dyn std::error::Error>> {
    let client = http_client();
    let url = format!("{}/api/user/self", base_url.trim_end_matches('/'));

    let mut req = super::super::apply_browser_headers(
        client.get(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Pragma", "no-cache")
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Sec-Fetch-Site", "same-origin")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Dest", "empty")
            .header("Referer", base_url.trim_end_matches('/')),
        profile,
    );

    if let Some(token) = access_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    if let Some(c) = cookie {
        req = req.header("Cookie", c);
    }
    if let Some(uid) = user_id {
        req = req
            .header("New-API-User", uid)
            .header("Veloera-User", uid)
            .header("X-Api-User", uid)
            .header("voapi-user", uid)
            .header("User-id", uid)
            .header("Rix-Api-User", uid)
            .header("neo-api-user", uid);
    }

    let response = req.send().await?;
    let status = response.status();
    let text = response.text().await?;

    // 解析 JSON
    let payload: Option<serde_json::Value> = serde_json::from_str(&text).ok();

    if !status.is_success() {
        tracing::error!("Balance fetch failed: HTTP {}, body: {}", status, &text);
        return Err(read_error_message(payload.as_ref())
            .unwrap_or_else(|| format!("余额请求失败：HTTP {}", status)).into());
    }

    // 尝试多种路径提取余额
    let quota = payload.as_ref()
        .and_then(|v| {
            // 尝试直接读取 quota
            read_number(v.get("quota"))
                // 尝试 data 字段（可能是对象或数字）
                .or_else(|| v.get("data").and_then(|d| {
                    if d.is_object() {
                        // data 是对象，尝试 data.quota
                        read_number(d.get("quota"))
                    } else {
                        // data 是数字
                        read_number(Some(d))
                    }
                }))
                // 尝试其他常见字段名
                .or_else(|| read_number(v.get("balance")))
                .or_else(|| read_number(v.get("credit")))
                .or_else(|| read_number(v.get("amount")))
        });

    if let Some(q) = quota {
        Ok(q)
    } else {
        // 安全截断，避免切断 UTF-8 多字节字符导致 panic
        let preview: String = text.chars().take(200).collect();
        tracing::error!("Balance field not found in response: {}", preview);
        Err(read_error_message(payload.as_ref())
            .unwrap_or_else(|| format!("余额请求失败：站点未返回余额字段。响应: {}", preview))
            .into())
    }
}
