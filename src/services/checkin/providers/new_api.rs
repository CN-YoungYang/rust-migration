use crate::error::Result;
use super::super::http_client;
use super::{CheckinResponse, classify_checkin_status};

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

pub async fn checkin(base_url: &str, token: &str, user_id: Option<&str>) -> Result<(String, String, Option<String>)> {
    let url = format!("{}/api/user/checkin", base_url.trim_end_matches('/'));
    let client = http_client();

    let mut req = client.post(&url)
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("Pragma", "no-cache")
        .body("{}");

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
    let status_code = response.status();
    let text = response.text().await?;

    if !status_code.is_success() {
        return Ok(("failed".to_string(), format!("HTTP {}", status_code), Some(text)));
    }

    let parsed: CheckinResponse = serde_json::from_str(&text)
        .unwrap_or(CheckinResponse {
            success: false,
            message: Some("Failed to parse response".into()),
        });

    let message = parsed.message.unwrap_or_else(|| "No message".to_string());
    let status = classify_checkin_status(parsed.success, &message);

    Ok((status.to_string(), message, Some(text)))
}

pub async fn fetch_balance(base_url: &str, user_id: Option<&str>, access_token: Option<&str>, cookie: Option<&str>) -> std::result::Result<f64, Box<dyn std::error::Error>> {
    let client = http_client();
    let url = format!("{}/api/user/self", base_url.trim_end_matches('/'));

    let mut req = client.get(&url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Pragma", "no-cache")
        .header("X-Requested-With", "XMLHttpRequest");

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
