use serde::{Deserialize, Serialize};
use crate::error::Result;
use super::super::http_client;
use super::format_awarded_quota;

#[derive(Debug, Serialize, Deserialize)]
pub struct X666Response {
    pub success: Option<bool>,
    pub message: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub quota: Option<f64>,
}

const DEFAULT_CHECKIN_URL: &str = "https://up.x666.me/api/checkin/spin";
const REFERER_URL: &str = "https://up.x666.me/";

fn normalize_message(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(v) => {
            if let Some(s) = v.as_str() {
                s.to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

fn is_already_checked_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    ["今日已签", "已签到", "已经签到", "already"]
        .iter()
        .any(|text| lower.contains(text))
}

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

/// 读取本次签到获得的额度（参考 Next.js readAwardedQuota: data.quota）
/// 依次尝试 data.quota / quota
fn read_awarded_quota(text: &str) -> Option<f64> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    read_number(value.get("data").and_then(|d| d.get("quota")))
        .or_else(|| read_number(value.get("quota")))
}

/// 将本次获得额度拼入消息（参考 Next.js runner.ts）
fn with_awarded_quota(message: String, text: &str) -> String {
    match read_awarded_quota(text) {
        Some(q) => format!("{}；本次获得额度：{}", message, format_awarded_quota(q)),
        None => message,
    }
}

pub async fn checkin(_base_url: &str, cookie: &str, custom_url: Option<&str>) -> Result<(String, String, Option<String>)> {
    let url = custom_url.unwrap_or(DEFAULT_CHECKIN_URL);
    let client = http_client();

    let response = client.post(url)
        .header("Cookie", cookie)
        .header("Accept", "*/*")
        .header("Accept-Language", "zh,zh-CN;q=0.9,en;q=0.8")
        .header("Origin", REFERER_URL.trim_end_matches("/"))
        .header("Referer", REFERER_URL)
        .send()
        .await?;

    let status_code = response.status();
    let text = response.text().await?;

    // 尝试解析 JSON，失败时创建包含原始文本的 payload
    let payload: Option<X666Response> = serde_json::from_str(&text).ok();
    
    // 如果 JSON 解析失败但有文本内容，将文本作为 message 处理
    let response_msg = if let Some(ref p) = payload {
        let msg = normalize_message(p.message.as_ref().or(p.error.as_ref()));
        if msg.is_empty() { 
            text.clone() 
        } else { 
            msg 
        }
    } else {
        text.clone()
    };

    // 先检查是否已签到（不管状态码）
    if is_already_checked_message(&response_msg) {
        return Ok(("already_checked".to_string(), with_awarded_quota(response_msg, &text), Some(text)));
    }

    // 检查 HTTP 状态码
    if !status_code.is_success() {
        return Ok(("failed".to_string(), format!("签到请求失败：HTTP {}", status_code), Some(text)));
    }

    // 检查 success 字段
    if payload.as_ref().and_then(|p| p.success).unwrap_or(false) {
        return Ok(("success".to_string(), with_awarded_quota(response_msg, &text), Some(text)));
    }

    // 默认失败
    Ok(("failed".to_string(), format!("签到失败：{}", response_msg), Some(text)))
}

pub async fn fetch_balance(cookie: Option<&str>) -> std::result::Result<f64, Box<dyn std::error::Error>> {
    let cookie = cookie.ok_or("Cookie is required for X666 balance query")?;
    let client = http_client();
    let url = "https://up.x666.me/api/checkin/status";

    let req = client.get(url)
        .header("Accept", "*/*")
        .header("Accept-Language", "zh,zh-CN;q=0.9,en;q=0.8")
        .header("Cookie", cookie)
        .header("Referer", REFERER_URL);

    let response = req.send().await?;
    let status = response.status();
    let text = response.text().await?;

    // 尝试解析 JSON
    let payload: Option<serde_json::Value> = serde_json::from_str(&text).ok();

    if !status.is_success() {
        tracing::error!("X666 balance fetch failed: HTTP {}, body: {}", status, &text);
        let message = payload.as_ref()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()))
            .unwrap_or("余额请求失败");
        return Err(format!("HTTP {}: {}", status, message).into());
    }

    // 尝试多种路径提取余额
    let quota = payload.as_ref()
        .and_then(|v| {
            // 尝试 current_quota
            read_number(v.get("current_quota"))
                // 尝试 quota
                .or_else(|| read_number(v.get("quota")))
                // 尝试 data.current_quota
                .or_else(|| v.get("data").and_then(|d| read_number(d.get("current_quota"))))
                // 尝试 data.quota
                .or_else(|| v.get("data").and_then(|d| read_number(d.get("quota"))))
                // 尝试其他字段
                .or_else(|| read_number(v.get("balance")))
                .or_else(|| read_number(v.get("credit")))
        });

    if let Some(q) = quota {
        Ok(q)
    } else {
        // 安全截断，避免切断 UTF-8 多字节字符导致 panic
        let preview: String = text.chars().take(200).collect();
        tracing::error!("X666 balance field not found in response: {}", preview);
        Err(format!("余额请求失败：站点未返回余额字段。响应: {}", preview).into())
    }
}
