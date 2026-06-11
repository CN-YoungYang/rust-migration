use reqwest::Client;
use serde::{Deserialize};
use crate::error::{Result};

#[derive(Debug, Deserialize)]
struct NewApiResponse {
    success: bool,
    message: Option<String>,
    data: Option<serde_json::Value>,
}

pub async fn checkin(base_url: &str, token: &str, user_id: Option<&str>) -> Result<(String, String, Option<String>)> {
    let url = format!("{}/api/user/checkin", base_url.trim_end_matches('/'));
    let client = Client::new();
    
    let mut req = client.post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json");
    
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
    
    let parsed: NewApiResponse = serde_json::from_str(&text)
        .unwrap_or(NewApiResponse {
            success: false,
            message: Some("Failed to parse response".into()),
            data: None,
        });
    
    let message = parsed.message.unwrap_or_else(|| "No message".to_string());
    let message_lower = message.to_lowercase();
    
    let status = if message_lower.contains("already") || message_lower.contains("��ǩ��") {
        "already_checked"
    } else if parsed.success || message_lower.contains("success") || message_lower.contains("�ɹ�") {
        "success"
    } else {
        "failed"
    };
    
    Ok((status.to_string(), message, Some(text)))
}
