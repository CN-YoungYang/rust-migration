use crate::{
    crypto,
    error::{AppError, Result},
    models::NotificationConfig,
    security::validate_public_http_url_resolved,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Duration},
};
use tokio_native_tls::{TlsConnector, TlsStream};

#[derive(Debug, Clone)]
pub struct NotificationPayload {
    pub account_name: String,
    pub site_type: String,
    pub base_url: String,
    pub status: String,
    pub message: String,
    pub balance: Option<f64>,
    pub consecutive_failures: i64,
}

struct SmtpMessage<'a> {
    host: &'a str,
    port: i64,
    username: &'a str,
    password: &'a str,
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    body: &'a str,
}

enum SmtpStream {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

impl SmtpStream {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            SmtpStream::Plain(stream) => stream.read(buf).await,
            SmtpStream::Tls(stream) => stream.read(buf).await,
        }
    }

    async fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            SmtpStream::Plain(stream) => stream.write_all(bytes).await,
            SmtpStream::Tls(stream) => stream.write_all(bytes).await,
        }
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        match self {
            SmtpStream::Plain(stream) => stream.flush().await,
            SmtpStream::Tls(stream) => stream.flush().await,
        }
    }
}

/// 发送通知
pub async fn send_notification(
    config: &NotificationConfig,
    payload: &NotificationPayload,
) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    match config.notify_type.as_str() {
        "email" => send_email_notification(config, payload).await,
        "webhook" => send_webhook_notification(config, payload).await,
        "telegram" => send_telegram_notification(config, payload).await,
        _ => Err(AppError::Validation(format!(
            "不支持的通知类型: {}",
            config.notify_type
        ))),
    }
}

/// 发送邮件通知
async fn send_email_notification(
    config: &NotificationConfig,
    payload: &NotificationPayload,
) -> Result<()> {
    let smtp_host = config
        .email_smtp_host
        .as_ref()
        .ok_or_else(|| AppError::Validation("邮件 SMTP 主机未配置".into()))?;
    let smtp_port = config
        .email_smtp_port
        .ok_or_else(|| AppError::Validation("邮件 SMTP 端口未配置".into()))?;
    let smtp_user = config
        .email_smtp_user
        .as_ref()
        .ok_or_else(|| AppError::Validation("邮件 SMTP 用户名未配置".into()))?;
    let smtp_password_enc = config
        .email_smtp_password
        .as_ref()
        .ok_or_else(|| AppError::Validation("邮件 SMTP 密码未配置".into()))?;
    let from = config
        .email_from
        .as_ref()
        .ok_or_else(|| AppError::Validation("发件人未配置".into()))?;
    let to = config
        .email_to
        .as_ref()
        .ok_or_else(|| AppError::Validation("收件人未配置".into()))?;

    let smtp_password = crypto::decrypt(smtp_password_enc)?;

    let subject = format!("[AI Hub] 签到提醒 - {}", payload.account_name);
    let body = format!(
        "账户名称：{}\n站点类型：{}\n站点地址：{}\n签到状态：{}\n通知信息：{}\n连续失败：{} 次\n余额：{}\n时间：{}",
        payload.account_name,
        payload.site_type,
        payload.base_url,
        payload.status,
        payload.message,
        payload.consecutive_failures,
        payload.balance.map(|b| format!("${:.2}", b)).unwrap_or_else(|| "未知".into()),
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
    );

    send_smtp(SmtpMessage {
        host: smtp_host,
        port: smtp_port,
        username: smtp_user,
        password: &smtp_password,
        from,
        to,
        subject: &subject,
        body: &body,
    })
    .await
}

async fn send_smtp(email: SmtpMessage<'_>) -> Result<()> {
    // 发送前再次按实际解析的 IP 做 SSRF 防护，与 webhook 路径保持一致，
    // 防止配置写入后 DNS 重绑定/TOCTOU 指向内网或元数据地址。
    let port = u16::try_from(email.port)
        .map_err(|_| AppError::Validation("SMTP 端口必须在 1~65535 之间".into()))?;
    crate::security::validate_public_host_resolved(email.host, port, "SMTP 主机").await?;
    if !(1..=65535).contains(&port) {
        return Err(AppError::Validation("SMTP 端口必须在 1~65535 之间".into()));
    }

    let addr = format!("{}:{}", email.host, port);
    let tcp_stream = timeout(Duration::from_secs(10), TcpStream::connect(&addr))
        .await
        .map_err(|_| AppError::Internal("连接 SMTP 服务器超时".into()))??;

    let mut stream = if email.port == 465 {
        SmtpStream::Tls(Box::new(connect_tls(email.host, tcp_stream).await?))
    } else {
        SmtpStream::Plain(tcp_stream)
    };

    expect_smtp(&mut stream, &[220]).await?;
    let ehlo = smtp_cmd(&mut stream, "EHLO ai-hub.local\r\n", &[250]).await?;

    if email.port != 465 {
        if !ehlo
            .lines()
            .any(|line| line.to_ascii_uppercase().contains("STARTTLS"))
        {
            return Err(AppError::Validation(
                "SMTP 服务器未提供 STARTTLS，拒绝明文认证".into(),
            ));
        }

        smtp_cmd(&mut stream, "STARTTLS\r\n", &[220]).await?;
        let plain_stream = match stream {
            SmtpStream::Plain(stream) => stream,
            SmtpStream::Tls(_) => {
                return Err(AppError::Internal("SMTP STARTTLS 状态异常".into()));
            }
        };
        stream = SmtpStream::Tls(Box::new(connect_tls(email.host, plain_stream).await?));
        smtp_cmd(&mut stream, "EHLO ai-hub.local\r\n", &[250]).await?;
    }

    smtp_cmd(&mut stream, "AUTH LOGIN\r\n", &[334]).await?;
    smtp_cmd(
        &mut stream,
        &format!("{}\r\n", general_purpose::STANDARD.encode(email.username)),
        &[334],
    )
    .await?;
    smtp_cmd(
        &mut stream,
        &format!("{}\r\n", general_purpose::STANDARD.encode(email.password)),
        &[235],
    )
    .await?;

    smtp_cmd(
        &mut stream,
        &format!("MAIL FROM:<{}>\r\n", email.from),
        &[250],
    )
    .await?;
    for recipient in email.to.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        smtp_cmd(
            &mut stream,
            &format!("RCPT TO:<{}>\r\n", recipient),
            &[250, 251],
        )
        .await?;
    }
    smtp_cmd(&mut stream, "DATA\r\n", &[354]).await?;

    let message = format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}\r\n.\r\n",
        email.from,
        email.to,
        email.subject,
        dot_stuff(email.body),
    );
    smtp_cmd(&mut stream, &message, &[250]).await?;
    let _ = smtp_cmd(&mut stream, "QUIT\r\n", &[221]).await;

    tracing::info!(
        "邮件通知发送成功: SMTP={}, From={}, To={}",
        addr,
        email.from,
        email.to
    );
    Ok(())
}

async fn connect_tls(host: &str, stream: TcpStream) -> Result<TlsStream<TcpStream>> {
    let connector = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| AppError::Internal(format!("创建 TLS 连接器失败: {}", e)))?;
    let connector = TlsConnector::from(connector);
    timeout(Duration::from_secs(10), connector.connect(host, stream))
        .await
        .map_err(|_| AppError::Internal("SMTP TLS 握手超时".into()))?
        .map_err(|e| AppError::Internal(format!("SMTP TLS 握手失败: {}", e)))
}

async fn smtp_cmd(stream: &mut SmtpStream, command: &str, expected: &[u16]) -> Result<String> {
    stream.write_all(command.as_bytes()).await?;
    stream.flush().await?;
    let response = read_smtp_response(stream).await?;
    validate_smtp_response(&response, expected)?;
    Ok(response)
}

async fn expect_smtp(stream: &mut SmtpStream, expected: &[u16]) -> Result<String> {
    let response = read_smtp_response(stream).await?;
    validate_smtp_response(&response, expected)?;
    Ok(response)
}

async fn read_smtp_response(stream: &mut SmtpStream) -> Result<String> {
    let mut response = String::new();
    let mut buf = [0_u8; 1024];

    loop {
        let n = timeout(Duration::from_secs(10), stream.read(&mut buf))
            .await
            .map_err(|_| AppError::Internal("读取 SMTP 响应超时".into()))??;
        if n == 0 {
            return Err(AppError::Internal("SMTP 连接已关闭".into()));
        }

        response.push_str(&String::from_utf8_lossy(&buf[..n]));
        if response
            .lines()
            .rev()
            .any(|line| line.len() >= 4 && line.as_bytes()[3] == b' ')
        {
            return Ok(response);
        }
    }
}

fn validate_smtp_response(response: &str, expected: &[u16]) -> Result<()> {
    let code = response
        .lines()
        .rev()
        .find(|line| line.len() >= 3)
        .and_then(|line| line.get(0..3))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| AppError::Internal(format!("无法解析 SMTP 响应: {}", response.trim())))?;

    if expected.contains(&code) {
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "SMTP 返回错误: {}",
            response.trim()
        )))
    }
}

fn dot_stuff(body: &str) -> String {
    body.lines()
        .map(|line| {
            if line.starts_with('.') {
                format!(".{}", line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// 发送 Webhook 通知
async fn send_webhook_notification(
    config: &NotificationConfig,
    payload: &NotificationPayload,
) -> Result<()> {
    let webhook_url = config
        .webhook_url
        .as_ref()
        .ok_or_else(|| AppError::Validation("Webhook URL 未配置".into()))?;
    validate_public_http_url_resolved(webhook_url, "Webhook URL").await?;
    let method = config.webhook_method.as_deref().unwrap_or("POST");

    let json_payload = json!({
        "type": "checkin_notification",
        "account_name": payload.account_name,
        "site_type": payload.site_type,
        "base_url": payload.base_url,
        "status": payload.status,
        "message": payload.message,
        "balance": payload.balance,
        "consecutive_failures": payload.consecutive_failures,
        "timestamp": Utc::now().to_rfc3339(),
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut request = match method {
        "POST" => client.post(webhook_url),
        "PUT" => client.put(webhook_url),
        _ => {
            return Err(AppError::Validation(format!(
                "不支持的 HTTP 方法: {}",
                method
            )))
        }
    };

    request = request.json(&json_payload);

    // 添加自定义 headers
    if let Some(headers_json) = &config.webhook_headers {
        if let Ok(headers) = serde_json::from_str::<serde_json::Value>(headers_json) {
            if let Some(obj) = headers.as_object() {
                for (key, value) in obj {
                    if let Some(val_str) = value.as_str() {
                        request = request.header(key, val_str);
                    }
                }
            }
        }
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        tracing::error!("Webhook 请求失败: {}", response.status());
        return Err(AppError::Internal(format!(
            "Webhook 返回错误: {}",
            response.status()
        )));
    }

    tracing::info!("Webhook 通知发送成功");
    Ok(())
}

/// 发送 Telegram 通知
async fn send_telegram_notification(
    config: &NotificationConfig,
    payload: &NotificationPayload,
) -> Result<()> {
    let bot_token_enc = config
        .telegram_bot_token
        .as_ref()
        .ok_or_else(|| AppError::Validation("Telegram Bot Token 未配置".into()))?;
    let chat_id = config
        .telegram_chat_id
        .as_ref()
        .ok_or_else(|| AppError::Validation("Telegram Chat ID 未配置".into()))?;

    let bot_token = crypto::decrypt(bot_token_enc)?;

    let text = format!(
        "*签到提醒*\n\n\
        📦 账户：`{}`\n\
        🌐 站点：{}\n\
        🔗 地址：{}\n\
        状态：{}\n\
        💬 信息：{}\n\
        🔁 连续失败：{} 次\n\
        💰 余额：{}\n\
        🕐 时间：{}",
        payload.account_name,
        payload.site_type,
        payload.base_url,
        payload.status,
        payload.message,
        payload.consecutive_failures,
        payload
            .balance
            .map(|b| format!("${:.2}", b))
            .unwrap_or_else(|| "未知".into()),
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
    );

    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = client
        .post(&url)
        .json(&json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown",
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        tracing::error!("Telegram 通知失败: {}", error_text);
        return Err(AppError::Internal(format!(
            "Telegram API 错误: {}",
            error_text
        )));
    }

    tracing::info!("Telegram 通知发送成功: chat_id={}", chat_id);
    Ok(())
}

/// 检查是否应该发送通知
pub fn should_notify(config: &NotificationConfig, payload: &NotificationPayload) -> bool {
    if !config.enabled {
        return false;
    }

    // 检查失败通知条件
    if config.on_failure
        && payload.status == "failed"
        && payload.consecutive_failures >= config.failure_threshold
    {
        return true;
    }

    // 检查余额过低条件
    if config.on_balance_low {
        if let (Some(balance), Some(threshold)) = (payload.balance, config.balance_threshold) {
            if balance < threshold {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> NotificationConfig {
        NotificationConfig {
            id: "n1".into(),
            owner_id: "u1".into(),
            notify_type: "webhook".into(),
            enabled: true,
            on_failure: true,
            failure_threshold: 2,
            on_balance_low: true,
            balance_threshold: Some(3.0),
            email_smtp_host: None,
            email_smtp_port: None,
            email_smtp_user: None,
            email_smtp_password: None,
            email_from: None,
            email_to: None,
            webhook_url: Some("https://example.com/hook".into()),
            webhook_method: Some("POST".into()),
            webhook_headers: None,
            telegram_bot_token: None,
            telegram_chat_id: None,
            note: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn payload(
        status: &str,
        consecutive_failures: i64,
        balance: Option<f64>,
    ) -> NotificationPayload {
        NotificationPayload {
            account_name: "account".into(),
            site_type: "new-api".into(),
            base_url: "https://example.com".into(),
            status: status.into(),
            message: "message".into(),
            balance,
            consecutive_failures,
        }
    }

    #[test]
    fn notifies_when_failure_threshold_is_reached() {
        assert!(should_notify(&config(), &payload("failed", 2, Some(10.0))));
    }

    #[test]
    fn suppresses_failure_before_threshold() {
        assert!(!should_notify(&config(), &payload("failed", 1, Some(10.0))));
    }

    #[test]
    fn notifies_when_balance_is_low() {
        assert!(should_notify(&config(), &payload("success", 0, Some(2.5))));
    }

    #[test]
    fn dot_stuff_escapes_lines_starting_with_dot() {
        assert_eq!(dot_stuff("a\n.b\n..c"), "a\r\n..b\r\n...c");
    }
}
