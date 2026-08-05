pub mod anyrouter;
pub mod new_api;
pub mod x666;

#[derive(Debug, serde::Deserialize)]
pub struct CheckinResponse {
    /// 缺省（如 `{code,message,data}` 结构的 fork）时为 None。
    /// 用 Option 区分“未提供”与“显式 false”：显式 false 必须被尊重，
    /// 不能被 code 兜底覆盖成成功（M10 回归修复）。
    #[serde(default)]
    pub success: Option<bool>,
    /// fork 自定义状态码（new-api 系常用 code=200 表示成功；微信/钉钉风格用 code=0）。
    /// 用宽松 Value 承接字符串/浮点等写法，避免字段类型不合导致整条响应解析失败。
    #[serde(default)]
    pub code: Option<serde_json::Value>,
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

/// 解析响应中的“成功”信号。`success` 字段显式给出时以它为准；
/// 仅当字段缺失时才用 `code` 兜底（0 / 200 两种 fork 约定的成功码）。
/// 显式 `success:false` 不被 code 覆盖，避免 `{success:false, code:200}`
/// 被误判为签到成功（M10 回归修复）。
pub fn resolve_checkin_success(parsed: &CheckinResponse) -> bool {
    match parsed.success {
        Some(true) => true,
        Some(false) => false,
        None => code_implies_success(parsed.code.as_ref()),
    }
}

/// `code` 字段是否表达成功：仅 0（微信/钉钉风格）与 200（HTTP 风格）两种
/// fork 约定视为成功，其余值一律按失败处理（fail-closed）。
/// 用 `read_number` 容忍字符串/浮点写法，如 `"200"`、`200.0`。
fn code_implies_success(code: Option<&serde_json::Value>) -> bool {
    matches!(read_number(code), Some(c) if c == 0.0 || c == 200.0)
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

/// 判断响应文本是否为 HTML 页面（非 JSON），避免把整页当消息或参与关键词判定（M11）
pub(crate) fn looks_like_html(text: &str) -> bool {
    text.trim_start().starts_with('<')
}

/// 从 HTML 错误页提取 `<title>` 文本（trim 后），用于把错误原因带进消息，
/// 如 Cloudflare 504 页的 "504: Gateway time-out"。提取不到返回 None。
/// 只读 ASCII 边界（`<title...>` 与 `</title` 均为单字节字符），故字节切片安全。
pub(crate) fn extract_html_title(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let title_start = lower.find("<title")?;
    let content_start = lower[title_start..].find('>')? + title_start + 1;
    let content_end = lower[content_start..].find("</title")? + content_start;
    let title = text[content_start..content_end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
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
        assert_eq!(parsed.success, None);
        assert_eq!(parsed.message.as_deref(), Some("ok"));
        assert_eq!(parsed.code, None);
    }

    #[test]
    fn checkin_response_parses_code_200_fork() {
        let parsed: CheckinResponse =
            serde_json::from_str(r#"{"code":200,"message":"ok"}"#).unwrap();
        assert_eq!(parsed.success, None);
        assert_eq!(parsed.code, Some(serde_json::json!(200)));
        assert!(resolve_checkin_success(&parsed));
    }

    #[test]
    fn explicit_success_false_is_authoritative_over_code() {
        // 回归：`{success:false, code:200}` 不得被 code 兜底覆盖为成功
        let parsed: CheckinResponse =
            serde_json::from_str(r#"{"success":false,"code":200,"message":"checkin failed"}"#)
                .unwrap();
        assert!(!resolve_checkin_success(&parsed));
    }

    #[test]
    fn code_heuristic_handles_zero_string_and_float_codes() {
        // 微信/钉钉风格 code=0 表示成功；字符串/浮点写法也能解析（不再整条解析失败）
        assert!(resolve_checkin_success(
            &serde_json::from_str(r#"{"code":0}"#).unwrap()
        ));
        assert!(resolve_checkin_success(
            &serde_json::from_str(r#"{"code":200}"#).unwrap()
        ));
        assert!(resolve_checkin_success(
            &serde_json::from_str(r#"{"code":"200"}"#).unwrap()
        ));
        assert!(resolve_checkin_success(
            &serde_json::from_str(r#"{"code":200.0}"#).unwrap()
        ));
        // 其余 code 一律按失败处理（fail-closed）
        assert!(!resolve_checkin_success(
            &serde_json::from_str(r#"{"code":500}"#).unwrap()
        ));
        // 显式 success:false 优先于 code=0
        assert!(!resolve_checkin_success(
            &serde_json::from_str(r#"{"success":false,"code":0}"#).unwrap()
        ));
    }

    #[test]
    fn looks_like_html_detects_pages() {
        assert!(looks_like_html("<!DOCTYPE html>\n<html>"));
        assert!(looks_like_html("<html><body>acw_sc__v2</body></html>"));
        assert!(!looks_like_html("{\"success\":true}"));
        assert!(!looks_like_html("plain text"));
        assert!(!looks_like_html(""));
    }

    #[test]
    fn extract_html_title_reads_cloudflare_error_page() {
        let html = "<!DOCTYPE html><html><head><title>56321654.xyz | 504: Gateway time-out</title></head></html>";
        assert_eq!(
            extract_html_title(html),
            Some("56321654.xyz | 504: Gateway time-out".to_string())
        );
    }

    #[test]
    fn extract_html_title_handles_attributes_case_and_whitespace() {
        assert_eq!(
            extract_html_title(
                "<html><head><TITLE class=\"x\">  Bad Gateway  </TITLE></head></html>"
            ),
            Some("Bad Gateway".to_string())
        );
        assert_eq!(
            extract_html_title("<title>\n  504: Gateway time-out\n</title>"),
            Some("504: Gateway time-out".to_string())
        );
    }

    #[test]
    fn extract_html_title_returns_none_for_non_html_or_missing() {
        assert_eq!(extract_html_title("{\"success\":true}"), None);
        assert_eq!(
            extract_html_title("<html><body>no title</body></html>"),
            None
        );
        assert_eq!(extract_html_title("<title></title>"), None);
        assert_eq!(extract_html_title(""), None);
    }
}
