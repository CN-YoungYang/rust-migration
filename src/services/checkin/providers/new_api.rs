use crate::error::Result;
use super::super::http_client;
use super::{CheckinResponse, classify_checkin_status};

pub async fn checkin(base_url: &str, token: &str, user_id: Option<&str>) -> Result<(String, String, Option<String>)> {
    let url = format!("{}/api/user/checkin", base_url.trim_end_matches('/'));
    let client = http_client();

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

    let mut req = client.get(&url);

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

    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status, text).into());
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let quota = json["data"]["quota"].as_f64()
        .or_else(|| json["data"]["remainQuota"].as_f64())
        .ok_or_else(|| format!("No quota field in balance response: {}", text))?;

    Ok(quota / 500000.0)
}
