use serde::{Deserialize, Serialize};
use crate::error::Result;
use super::super::http_client;

#[derive(Debug, Serialize, Deserialize)]
pub struct X666Response {
    pub success: Option<bool>,
    pub message: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub quota: Option<f64>,
}

const DEFAULT_CHECKIN_URL: &str = "https://up.x666.me/api/checkin/spin";
const REFERER_URL: &str = "https://up.x666.me/";

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

    let payload: Option<X666Response> = serde_json::from_str(&text).ok();
    let response_msg = payload.as_ref()
        .and_then(|p| p.message.as_ref().or(p.error.as_ref()))
        .and_then(|v| v.as_str())
        .unwrap_or(&text)
        .to_string();

    let msg_lower = response_msg.to_lowercase();
    let is_already = ["今日已签", "已签到", "已经签到", "already"].iter()
        .any(|s| msg_lower.contains(&s.to_lowercase()));

    if is_already {
        return Ok(("already_checked".to_string(), response_msg, Some(text)));
    }

    if !status_code.is_success() {
        return Ok(("failed".to_string(), format!("签到请求失败：HTTP {}", status_code), Some(text)));
    }

    if payload.as_ref().and_then(|p| p.success).unwrap_or(false) {
        return Ok(("success".to_string(), response_msg, Some(text)));
    }

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
        .header("Referer", "https://up.x666.me/");

    let response = req.send().await?;
    let status = response.status();
    let text = response.text().await?;

    let payload: Option<serde_json::Value> = serde_json::from_str(&text).ok();

    if !status.is_success() {
        let message = payload.as_ref()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()))
            .unwrap_or("");
        return Err(message.to_string().into());
    }

    let quota = payload.as_ref()
        .and_then(|v| v.get("current_quota"))
        .and_then(|v| v.as_f64()
            .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok())))
        .ok_or_else(|| "余额请求失败：站点未返回 current_quota".to_string())?;

    Ok(quota)
}
