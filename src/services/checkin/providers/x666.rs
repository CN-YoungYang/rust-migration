use super::super::{http_client, resolve_checkin_url, BrowserProfile};
use super::{
    extract_html_title, format_awarded_quota, is_already_checked_message, looks_like_html,
    read_number,
};
use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct X666Response {
    pub success: Option<bool>,
    pub message: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    /// 用宽松类型承接，避免站点把 quota 返回为字符串时整条解析失败（L4）
    #[serde(default)]
    pub quota: Option<serde_json::Value>,
}

const DEFAULT_CHECKIN_PATH: &str = "/api/checkin/spin";
const DEFAULT_BALANCE_PATH: &str = "/api/checkin/status";
const DEFAULT_BASE_URL: &str = "https://up.x666.me";

fn join_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn normalize_message(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(v) => {
            if let Some(s) = v.as_str() {
                s.to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

/// 读取本次签到获得的额度（参考 Next.js readAwardedQuota: data.quota）
/// 依次尝试 data.quota / quota
fn read_awarded_quota(text: &str) -> Option<f64> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    read_number(value.get("data").and_then(|d| d.get("quota")))
        .or_else(|| read_number(value.get("quota")))
}

/// 将本次获得额度拼入消息（参考 Next.js runner.ts）
fn with_awarded_quota(message: String, text: &str) -> String {
    match read_awarded_quota(text) {
        Some(q) => format!("{}；本次获得额度：{}", message, format_awarded_quota(q)),
        None => message,
    }
}

pub async fn checkin(
    base_url: &str,
    cookie: &str,
    custom_url: Option<&str>,
    profile: &BrowserProfile,
) -> Result<(String, String, Option<String>)> {
    let effective_base = if base_url.is_empty() {
        DEFAULT_BASE_URL
    } else {
        base_url
    };
    let url = resolve_checkin_url(effective_base, custom_url, DEFAULT_CHECKIN_PATH)?;
    let referer = format!("{}/", effective_base.trim_end_matches('/'));
    let client = http_client();

    let req = super::super::apply_browser_headers(
        client
            .post(&url)
            .header("Cookie", cookie)
            .header("Accept", "*/*")
            .header("Origin", effective_base.trim_end_matches('/'))
            .header("Referer", &referer)
            // x666 签到是同源 fetch 请求
            .header("Sec-Fetch-Site", "same-origin")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Dest", "empty"),
        profile,
    );

    let response = req.send().await?;

    let status_code = response.status();
    let text = response.text().await?;

    // 尝试解析 JSON，失败时创建包含原始文本的 payload
    let payload: Option<X666Response> = serde_json::from_str(&text).ok();

    // 提取站点消息：优先 JSON 的 message/error 字段；JSON 解析失败或字段为空时，
    // 仅当文本不是 HTML 页面才回退为原始文本（避免把整页 HTML 当消息，M11）；
    // HTML 错误页则提取 <title> 带出错误原因，如 "站点返回错误页：504: Gateway time-out"。
    let response_msg = if let Some(ref p) = payload {
        let msg = normalize_message(p.message.as_ref().or(p.error.as_ref()));
        if msg.is_empty() && !looks_like_html(&text) {
            text.clone()
        } else {
            msg
        }
    } else if looks_like_html(&text) {
        extract_html_title(&text)
            .map(|t| format!("站点返回错误页：{}", t))
            .unwrap_or_default()
    } else {
        text.clone()
    };

    // 先检查是否已签到（空消息天然不命中关键词，不会误判）
    if is_already_checked_message(&response_msg) {
        return Ok((
            "already_checked".to_string(),
            with_awarded_quota(response_msg, &text),
            Some(text),
        ));
    }

    // 检查 HTTP 状态码（L5：非 2xx 优先用站点返回的错误消息）
    if !status_code.is_success() {
        let message = if response_msg.is_empty() {
            format!("签到请求失败：HTTP {}", status_code)
        } else {
            response_msg
        };
        return Ok(("failed".to_string(), message, Some(text)));
    }

    // 检查 success 字段
    if payload.as_ref().and_then(|p| p.success).unwrap_or(false) {
        return Ok((
            "success".to_string(),
            with_awarded_quota(response_msg, &text),
            Some(text),
        ));
    }

    // 默认失败
    Ok((
        "failed".to_string(),
        if response_msg.is_empty() {
            "签到失败：站点未返回成功状态".to_string()
        } else {
            format!("签到失败：{}", response_msg)
        },
        Some(text),
    ))
}

pub async fn fetch_balance(
    base_url: Option<&str>,
    cookie: Option<&str>,
    profile: &BrowserProfile,
) -> std::result::Result<f64, Box<dyn std::error::Error>> {
    let cookie = cookie.ok_or("X666 余额查询必须填写 Cookie")?;
    let effective_base = base_url
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_BASE_URL);
    let url = join_url(effective_base, DEFAULT_BALANCE_PATH);
    let referer = format!("{}/", effective_base.trim_end_matches('/'));
    let client = http_client();

    let req = super::super::apply_browser_headers(
        client
            .get(&url)
            .header("Accept", "*/*")
            .header("Cookie", cookie)
            .header("Referer", &referer)
            .header("Sec-Fetch-Site", "same-origin")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Dest", "empty"),
        profile,
    );

    let response = req.send().await?;
    let status = response.status();
    let text = response.text().await?;

    // 尝试解析 JSON
    let payload: Option<serde_json::Value> = serde_json::from_str(&text).ok();

    if !status.is_success() {
        tracing::error!(
            "X666 balance fetch failed: HTTP {}, body: {}",
            status,
            &text
        );
        let message = payload
            .as_ref()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()))
            .unwrap_or("余额请求失败");
        return Err(format!("HTTP {}: {}", status, message).into());
    }

    // 尝试多种路径提取余额
    let quota = payload.as_ref().and_then(|v| {
        // 尝试 current_quota
        read_number(v.get("current_quota"))
            // 尝试 quota
            .or_else(|| read_number(v.get("quota")))
            // 尝试 data.current_quota
            .or_else(|| {
                v.get("data")
                    .and_then(|d| read_number(d.get("current_quota")))
            })
            // 尝试 data.quota
            .or_else(|| v.get("data").and_then(|d| read_number(d.get("quota"))))
            // 尝试其他字段
            .or_else(|| read_number(v.get("balance")))
            .or_else(|| read_number(v.get("credit")))
    });

    if let Some(q) = quota {
        Ok(q)
    } else {
        // 安全截断，避免切断 UTF-8 多字节字符导致 panic（预览仅入日志，不回显到用户消息）
        let preview: String = text.chars().take(200).collect();
        tracing::error!("X666 balance field not found in response: {}", preview);
        Err("余额请求失败：站点未返回余额字段".to_string().into())
    }
}
