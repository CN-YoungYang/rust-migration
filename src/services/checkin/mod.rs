pub mod providers;
pub mod runner;

use rand::seq::SliceRandom;
use rand::Rng;
use reqwest::Client;
use std::sync::OnceLock;

pub static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// 一组自洽的浏览器指纹：User-Agent 与配套的 sec-ch-ua / 平台 / 语言保持一致，
/// 否则 Cloudflare 等 WAF 会因“UA 声称是 Chrome 但缺少客户端提示”而扣分。
/// 防判定核心：每个账户随机选一个完整 profile，避免同 IP + 同指纹的关联。
pub struct BrowserProfile {
    pub user_agent: &'static str,
    /// sec-ch-ua 头值（仅 Chromium 系有；Firefox 为 None）
    pub sec_ch_ua: Option<&'static str>,
    /// sec-ch-ua-platform 头值
    pub sec_ch_ua_platform: &'static str,
    /// Accept-Language 头值
    pub accept_language: &'static str,
}

/// 近期版本的浏览器指纹池（2026 年在用大版本），Chrome/Firefox/Edge 各占一定比例。
/// 版本号尽量贴近“当前主流”，落后两年的 UA 是 WAF 的强 bot 信号。
const BROWSER_PROFILES: &[BrowserProfile] = &[
    // Chrome 135 (Windows)
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36",
        sec_ch_ua: Some(r#""Google Chrome";v="135", "Chromium";v="135", "Not-A.Brand";v="99""#),
        sec_ch_ua_platform: r#""Windows""#,
        accept_language: "zh-CN,zh;q=0.9,en;q=0.8",
    },
    // Chrome 134 (Windows)
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
        sec_ch_ua: Some(r#""Google Chrome";v="134", "Chromium";v="134", "Not-A.Brand";v="99""#),
        sec_ch_ua_platform: r#""Windows""#,
        accept_language: "zh-CN,zh;q=0.9,en;q=0.8",
    },
    // Chrome 135 (macOS)
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36",
        sec_ch_ua: Some(r#""Google Chrome";v="135", "Chromium";v="135", "Not-A.Brand";v="99""#),
        sec_ch_ua_platform: r#""macOS""#,
        accept_language: "zh-CN,zh;q=0.9,en;q=0.8",
    },
    // Firefox 137 (Windows) —— 无 sec-ch-ua
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:137.0) Gecko/20100101 Firefox/137.0",
        sec_ch_ua: None,
        sec_ch_ua_platform: r#""Windows""#,
        accept_language: "zh-CN,zh;q=0.9,en;q=0.8",
    },
    // Edge 135 (Windows)
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36 Edg/135.0.0.0",
        sec_ch_ua: Some(r#""Microsoft Edge";v="135", "Chromium";v="135", "Not-A.Brand";v="99""#),
        sec_ch_ua_platform: r#""Windows""#,
        accept_language: "zh-CN,zh;q=0.9,en;q=0.8",
    },
];

/// 随机选一个浏览器指纹 profile（线程安全）。
pub fn random_browser_profile() -> &'static BrowserProfile {
    BROWSER_PROFILES
        .choose(&mut rand::thread_rng())
        .expect("BROWSER_PROFILES is non-empty")
}

/// 把一组自洽的浏览器 header 应用到请求上，降低被 WAF 判定为 bot 的概率：
/// - User-Agent（覆盖 client 单例默认 UA）
/// - sec-ch-ua / sec-ch-ua-mobile / sec-ch-ua-platform（Chromium 客户端提示）
/// - Accept-Language（缺失是最常见的 bot 特征之一）
///
/// 注意：Sec-Fetch-* 和 Referer 因请求上下文而异，由调用方按需补。
pub fn apply_browser_headers(
    req: reqwest::RequestBuilder,
    profile: &BrowserProfile,
) -> reqwest::RequestBuilder {
    let mut r = req
        .header(reqwest::header::USER_AGENT, profile.user_agent)
        .header("Accept-Language", profile.accept_language)
        .header("sec-ch-ua-platform", profile.sec_ch_ua_platform)
        .header("sec-ch-ua-mobile", "?0");
    if let Some(ch) = profile.sec_ch_ua {
        r = r.header("sec-ch-ua", ch);
    }
    r
}

/// 根据管理员设置的 batchDelayMin/Max（秒）生成一个随机延迟。
/// min/max 非法或 min>max 时返回 None（表示不延迟）。
/// 防判定：批量/定时签到时相邻账户之间错开请求时间。
pub fn random_delay_secs(min: i32, max: i32) -> Option<u64> {
    if max <= 0 {
        return None;
    }
    let lo = min.max(0).min(max) as u64;
    let hi = max as u64;
    if hi <= lo {
        return Some(hi);
    }
    Some(rand::thread_rng().gen_range(lo..=hi))
}
