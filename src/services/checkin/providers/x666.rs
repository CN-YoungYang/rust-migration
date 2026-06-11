use reqwest::Client;
use crate::error::{Result};

pub async fn checkin(base_url: &str, cookie: &str) -> Result<(String, String, Option<String>)> {
    let url = format!("{}/user/checkin", base_url.trim_end_matches('/'));
    let client = Client::new();
    
    let response = client.post(&url)
        .header("Cookie", cookie)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    
    let status_code = response.status();
    let text = response.text().await?;
    
    if !status_code.is_success() {
        return Ok(("failed".to_string(), format!("HTTP {}", status_code), Some(text)));
    }
    
    let text_lower = text.to_lowercase();
    
    let status = if text_lower.contains("already") || text_lower.contains("已签到") {
        "already_checked"
    } else if text_lower.contains("success") || text_lower.contains("成功") {
        "success"
    } else {
        "failed"
    };
    
    Ok((status.to_string(), text.clone(), Some(text)))
}
