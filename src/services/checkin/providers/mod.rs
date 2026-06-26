pub mod anyrouter;
pub mod new_api;
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

/// 判断消息是否表示今日已签到（供 x666/anyrouter 使用）
pub fn is_already_checked_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    ["今日已签", "已签到", "已经签到", "今天已经签到", "already"]
        .iter()
        .any(|text| lower.contains(text))
}

/// 从 JSON 值中读取数字（支持字符串和数值类型）
pub fn read_number(value: Option<&serde_json::Value>) -> Option<f64> {
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

/// 从 JSON payload 中读取错误消息（尝试 message / error 字段）
pub fn read_error_message(payload: Option<&serde_json::Value>) -> Option<String> {
    payload
        .and_then(|v| v.get("message").or_else(|| v.get("error")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// One API / New API 系列标准换算：500000 quota = 1 美元
/// 与 Next.js 版本 (QUOTA_PER_USD = 500000) 保持一致
const QUOTA_PER_USD: f64 = 500_000.0;

/// 格式化本次签到获得的额度（参考 Next.js runner.ts formatAwardedQuota）
pub fn format_awarded_quota(quota: f64) -> String {
    let usd = quota / QUOTA_PER_USD;
    format!("{} quota（约 ${:.2}）", quota as i64, usd)
}
