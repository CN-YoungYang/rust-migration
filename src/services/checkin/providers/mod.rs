pub mod new_api;
pub mod anyrouter;
pub mod x666;

#[derive(Debug, serde::Deserialize)]
pub struct CheckinResponse {
    pub success: bool,
    pub message: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

pub fn classify_checkin_status(success: bool, message: &str) -> &'static str {
    let lower = message.to_lowercase();
    if lower.contains("already")
        || lower.contains("已签")
        || lower.contains("已经签到")
        || lower.contains("今天已经签到")
    {
        "already_checked"
    } else if success || lower.contains("success") || lower.contains("成功") {
        "success"
    } else {
        "failed"
    }
}

/// One API / New API 系列标准换算：500000 quota = 1 美元
/// 与 Next.js 版本 (QUOTA_PER_USD = 500000) 保持一致
const QUOTA_PER_USD: f64 = 500_000.0;

/// 格式化本次签到获得的额度（参考 Next.js runner.ts formatAwardedQuota）
pub fn format_awarded_quota(quota: f64) -> String {
    let usd = quota / QUOTA_PER_USD;
    format!("{} quota（约 ${:.2}）", quota as i64, usd)
}
