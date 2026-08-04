pub mod anyrouter;
pub mod new_api;
pub mod x666;

#[derive(Debug, serde::Deserialize)]
pub struct CheckinResponse {
    /// 缺省（如 `{code,message,data}` 结构的 fork）时为 false，避免整条解析失败
    #[serde(default)]
    pub success: bool,
    /// HTTP 风格成功码（部分 new-api fork 用 code=200 表示成功）
    #[serde(default)]
    pub code: Option<i64>,
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
    } else if success {
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

/// 从 JSON 值中读取数字（支持字符串和数值类型），并过滤 NaN/Infinity
pub fn read_number(value: Option<&serde_json::Value>) -> Option<f64> {
    let v = value?;
    if let Some(n) = v.as_f64() {
        return n.is_finite().then_some(n);
    }
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            if let Ok(n) = trimmed.parse::<f64>() {
                if n.is_finite() {
                    return Some(n);
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_success_bool_authoritatively() {
        assert_eq!(classify_checkin_status(true, "签到成功"), "success");
        assert_eq!(classify_checkin_status(true, "OK"), "success");
        // M9：仅信任 success 布尔值，不因消息含 success/成功 关键词而升级为成功
        assert_eq!(classify_checkin_status(false, "not success"), "failed");
        assert_eq!(classify_checkin_status(false, "签到失败"), "failed");
    }

    #[test]
    fn classifies_already_checked_keywords() {
        assert_eq!(
            classify_checkin_status(false, "今日已签到"),
            "already_checked"
        );
        assert_eq!(
            classify_checkin_status(false, "已经签到过啦"),
            "already_checked"
        );
        assert_eq!(
            classify_checkin_status(false, "already checked today"),
            "already_checked"
        );
    }

    #[test]
    fn already_checked_message_matches_chinese_and_english() {
        assert!(is_already_checked_message("今日已签"));
        assert!(is_already_checked_message("您今天已经签到过了"));
        assert!(is_already_checked_message("already"));
        assert!(!is_already_checked_message("签到成功"));
        assert!(!is_already_checked_message(""));
    }

    #[test]
    fn read_number_parses_numeric_and_string() {
        assert_eq!(read_number(Some(&serde_json::json!(123))), Some(123.0));
        assert_eq!(read_number(Some(&serde_json::json!("456"))), Some(456.0));
        assert_eq!(read_number(Some(&serde_json::json!(" 78.5 "))), Some(78.5));
        assert_eq!(read_number(Some(&serde_json::json!("abc"))), None);
        assert_eq!(read_number(None), None);
    }

    #[test]
    fn read_number_rejects_non_finite() {
        // L7：NaN/Infinity 会污染 lastBalance 显示为 约 $NaN
        assert_eq!(read_number(Some(&serde_json::json!("NaN"))), None);
        assert_eq!(read_number(Some(&serde_json::json!("Infinity"))), None);
        assert_eq!(read_number(Some(&serde_json::json!("-inf"))), None);
    }

    #[test]
    fn checkin_response_parses_without_success_field() {
        // M10：缺省 success 的 fork 响应不再整条解析失败
        let parsed: CheckinResponse = serde_json::from_str(r#"{"message":"ok"}"#).unwrap();
        assert!(!parsed.success);
        assert_eq!(parsed.message.as_deref(), Some("ok"));
        assert_eq!(parsed.code, None);
    }

    #[test]
    fn checkin_response_parses_code_200_fork() {
        let parsed: CheckinResponse =
            serde_json::from_str(r#"{"code":200,"message":"ok"}"#).unwrap();
        assert!(!parsed.success);
        assert_eq!(parsed.code, Some(200));
    }
}
