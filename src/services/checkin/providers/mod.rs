pub mod new_api;
pub mod anyrouter;
pub mod x666;

#[derive(Debug, serde::Deserialize)]
pub struct CheckinResponse {
    pub success: bool,
    pub message: Option<String>,
}

pub fn classify_checkin_status(success: bool, message: &str) -> &'static str {
    let lower = message.to_lowercase();
    if lower.contains("already") || lower.contains("已签") {
        "already_checked"
    } else if success || lower.contains("success") || lower.contains("成功") {
        "success"
    } else {
        "failed"
    }
}
