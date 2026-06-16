pub mod runner;
pub mod providers;

use std::sync::OnceLock;
use reqwest::Client;
use rand::seq::SliceRandom;
use rand::Rng;

pub static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// 真实浏览器 UA 池（Chrome / Firefox / Edge），用于防判定：
/// 多账户签到时每个账户随机选一个，降低“同 IP + 同 UA”关联。
const USER_AGENTS: &[&str] = &[
    // Chrome (Windows)
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    // Chrome (macOS)
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    // Firefox (Windows)
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0",
    // Edge (Windows)
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
];

/// 从 UA 池随机选一个浏览器 UA（线程安全，每次调用返回 'static 借用）。
pub fn random_user_agent() -> &'static str {
    USER_AGENTS
        .choose(&mut rand::thread_rng())
        .expect("USER_AGENTS is non-empty")
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
