use crate::error::Result;
use super::super::http_client;
use super::{CheckinResponse, classify_checkin_status};

pub async fn checkin(base_url: &str, token: &str) -> Result<(String, String, Option<String>)> {
    let url = format!("{}/api/user/checkin", base_url.trim_end_matches('/'));
    let client = http_client();

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

    let parsed: CheckinResponse = serde_json::from_str(&text)
        .unwrap_or(CheckinResponse {
            success: false,
            message: Some("Failed to parse response".into()),
        });

    let message = parsed.message.unwrap_or_else(|| "No message".to_string());
    let status = classify_checkin_status(parsed.success, &message);

    Ok((status.to_string(), message, Some(text)))
}
