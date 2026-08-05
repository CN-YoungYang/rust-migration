use crate::error::{AppError, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::net::lookup_host;

/// 检查 URL 是否指向内网/私有地址。
/// 拒绝 127.0.0.0/8、10.0.0.0/8、172.16.0.0/12、192.168.0.0/16、
/// 169.254.0.0/16、0.0.0.0、localhost、::1 等。
pub fn is_private_url(url: &str) -> bool {
    let host = match reqwest::Url::parse(url) {
        Ok(u) => u.host_str().unwrap_or("").to_lowercase(),
        Err(_) => return true,
    };

    if host == "localhost" || host.ends_with(".local") || host.ends_with(".internal") {
        return true;
    }

    if host == "::1" || host == "[::1]" {
        return true;
    }

    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return is_private_ip(IpAddr::V4(ip));
    }

    if let Ok(ip) = host
        .trim_matches(|c| c == '[' || c == ']')
        .parse::<Ipv6Addr>()
    {
        return is_private_ip(IpAddr::V6(ip));
    }

    false
}

pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets: [u8; 4] = ip.octets();
            matches!(
                octets,
                [127, ..]
                    | [10, ..]
                    | [192, 168, ..]
                    | [169, 254, ..]
                    | [0, 0, 0, 0]
                    | [100, 64..=127, ..]
                    | [198, 18..=19, ..]
                    | [224..=255, ..]
            ) || matches!(octets, [172, b, ..] if (16..=31).contains(&b))
                || ip.is_broadcast()
                || ip.is_documentation()
        }
        IpAddr::V6(ip) => {
            // IPv4-mapped（::ffff:a.b.c.d）与 IPv4-compatible（::a.b.c.d）地址
            // 必须映射回 IPv4 后按 IPv4 私网规则判定，否则 ::ffff:127.0.0.1、
            // ::ffff:169.254.169.254 等会被当作公网地址绕过 SSRF 防护。
            if let Some(v4) = ip.to_ipv4() {
                return is_private_ip(IpAddr::V4(v4));
            }
            let segments = ip.segments();
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
        }
    }
}

pub fn validate_public_http_url(url: &str, field_name: &str) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError::Validation(format!(
            "{} 必须以 http:// 或 https:// 开头",
            field_name
        )));
    }
    if is_private_url(url) {
        return Err(AppError::Validation(format!(
            "{} 不能指向内网/私有地址（SSRF 防护）",
            field_name
        )));
    }
    Ok(())
}

pub async fn validate_public_http_url_resolved(url: &str, field_name: &str) -> Result<()> {
    validate_public_http_url(url, field_name)?;
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| AppError::Validation(format!("{} URL 格式无效", field_name)))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::Validation(format!("{} 缺少主机名", field_name)))?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| AppError::Validation(format!("{} 缺少端口或协议不受支持", field_name)))?;
    validate_public_host_resolved(host, port, field_name).await
}

pub fn validate_public_host(host: &str, field_name: &str) -> Result<()> {
    let normalized = host.trim();
    if normalized.is_empty() {
        return Err(AppError::Validation(format!("{} 不能为空", field_name)));
    }
    let probe_url = format!("smtp://{}", normalized);
    if is_private_url(&probe_url) {
        return Err(AppError::Validation(format!(
            "{} 不能指向内网/私有地址（SSRF 防护）",
            field_name
        )));
    }
    Ok(())
}

pub async fn validate_public_host_resolved(host: &str, port: u16, field_name: &str) -> Result<()> {
    validate_public_host(host, field_name)?;
    // DNS 解析加超时：解析器慢/丢包时不能无限挂起，否则批量/定时签到会卡死在
    // 第一个账户的 SSRF 复核上（前端表现为"批量签到 0/N"长期不动）。
    let addrs = tokio::time::timeout(std::time::Duration::from_secs(5), lookup_host((host, port)))
        .await
        .map_err(|_| {
            AppError::Validation(format!("{} DNS 解析超时，无法确认目标是否安全", field_name))
        })?
        .map_err(|_| {
            AppError::Validation(format!("{} DNS 解析失败，无法确认目标是否安全", field_name))
        })?;

    let mut resolved_any = false;
    for addr in addrs {
        resolved_any = true;
        if is_private_ip(addr.ip()) {
            return Err(AppError::Validation(format!(
                "{} 解析到内网/私有地址（SSRF 防护）",
                field_name
            )));
        }
    }

    if !resolved_any {
        return Err(AppError::Validation(format!(
            "{} DNS 未返回可用地址",
            field_name
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_private_http_urls() {
        assert!(validate_public_http_url("http://127.0.0.1:8080", "url").is_err());
        assert!(validate_public_http_url("http://10.0.0.2", "url").is_err());
        assert!(validate_public_http_url("http://localhost", "url").is_err());
    }

    #[test]
    fn accepts_public_http_urls() {
        assert!(validate_public_http_url("https://example.com", "url").is_ok());
    }

    #[test]
    fn rejects_private_smtp_hosts() {
        assert!(validate_public_host("127.0.0.1", "SMTP 主机").is_err());
        assert!(validate_public_host("localhost", "SMTP 主机").is_err());
    }

    #[test]
    fn classifies_non_public_ip_ranges_as_private() {
        assert!(is_private_ip("100.64.0.1".parse().unwrap()));
        assert!(is_private_ip("198.18.0.1".parse().unwrap()));
        assert!(is_private_ip("224.0.0.1".parse().unwrap()));
        assert!(is_private_ip("ff02::1".parse().unwrap()));
    }

    #[test]
    fn classifies_ipv4_mapped_ipv6_as_private() {
        // IPv4-mapped IPv6 必须按映射后的 IPv4 判定，防止 SSRF 绕过
        assert!(is_private_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("::ffff:169.254.169.254".parse().unwrap()));
        assert!(is_private_ip("::ffff:10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("::ffff:192.168.1.1".parse().unwrap()));
        // 公网映射地址仍应视为公网
        assert!(!is_private_ip("::ffff:8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("::ffff:1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn classifies_ipv4_compatible_ipv6_as_private() {
        assert!(is_private_ip("::127.0.0.1".parse().unwrap()));
        assert!(!is_private_ip("::8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn rejects_ipv4_mapped_ipv6_urls() {
        assert!(validate_public_http_url("http://[::ffff:127.0.0.1]:8080", "url").is_err());
        assert!(validate_public_http_url("http://[::ffff:169.254.169.254]", "url").is_err());
    }

    #[tokio::test]
    async fn resolved_http_validation_rejects_literal_private_ip() {
        assert!(
            validate_public_http_url_resolved("http://127.0.0.1:8080", "url")
                .await
                .is_err()
        );
    }
}
