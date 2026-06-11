use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::error::{Result, AppError};

#[derive(Debug, Deserialize, Serialize)]
struct AnyrouterResponse {
    success: bool,
    message: Option<String>,
}

pub async fn checkin(base_url: &str, token: &str) -> Result<(String, String, Option<String>)> {
    let url = format!("{}/api/user/checkin", base_url.trim_end_matches('/'));
    let client = Client::new();
    
    let response = client.post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .send()
        .await?;
    
    let status_code = response.status();
    let text = response.text().await?;
    
    if !status_code.is_success() {
        return Ok(("failed".to_string(), format!("HTTP {}", status_code), Some(text)));
    }
    
    let parsed: AnyrouterResponse = serde_json::from_str(&text)
        .unwrap_or(AnyrouterResponse {
            success: false,
            message: Some("Failed to parse response".into()),
        });
    
    let message = parsed.message.unwrap_or_else(|| "No message".to_string());
    let message_lower = message.to_lowercase();
    
    let status = if message_lower.contains("already") || message_lower.contains("已签到") {
        "already_checked"
    } else if parsed.success || message_lower.contains("success") || message_lower.contains("成功") {
        "success"
    } else {
        "failed"
    };
    
    Ok((status.to_string(), message, Some(text)))
}
