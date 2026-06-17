use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AppUser {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing, rename = "passwordHash")]
    #[sqlx(rename = "passwordHash")]
    pub password_hash: String,
    pub role: String,
    pub enabled: bool,
    pub note: Option<String>,
    #[serde(rename = "createdAt")]
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct CheckinAccount {
    pub id: String,
    pub name: String,
    #[serde(rename = "siteType")]
    #[sqlx(rename = "siteType")]
    pub site_type: String,
    #[serde(rename = "baseUrl")]
    #[sqlx(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "userId")]
    #[sqlx(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "ownerId")]
    #[sqlx(rename = "ownerId")]
    pub owner_id: Option<String>,
    #[serde(rename = "authType")]
    #[sqlx(rename = "authType")]
    pub auth_type: String,
    #[serde(skip_serializing, rename = "accessTokenEnc")]
    #[sqlx(rename = "accessTokenEnc")]
    pub access_token_enc: Option<String>,
    #[serde(skip_serializing, rename = "cookieEnc")]
    #[sqlx(rename = "cookieEnc")]
    pub cookie_enc: Option<String>,
    #[serde(rename = "customCheckinUrl")]
    #[sqlx(rename = "customCheckinUrl")]
    pub custom_checkin_url: Option<String>,
    pub enabled: bool,
    #[serde(rename = "retryEnabled")]
    #[sqlx(rename = "retryEnabled")]
    pub retry_enabled: bool,
    #[serde(rename = "lastBalance")]
    #[sqlx(rename = "lastBalance")]
    pub last_balance: Option<f64>,
    #[serde(rename = "lastBalanceAt")]
    #[sqlx(rename = "lastBalanceAt")]
    pub last_balance_at: Option<DateTime<Utc>>,
    #[serde(rename = "lastStatus")]
    #[sqlx(rename = "lastStatus")]
    pub last_status: Option<String>,
    #[serde(rename = "lastMessage")]
    #[sqlx(rename = "lastMessage")]
    pub last_message: Option<String>,
    #[serde(rename = "lastRunAt")]
    #[sqlx(rename = "lastRunAt")]
    pub last_run_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    #[serde(rename = "createdAt")]
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct CheckinRun {
    pub id: String,
    #[serde(rename = "accountId")]
    #[sqlx(rename = "accountId")]
    pub account_id: String,
    pub status: String,
    pub message: Option<String>,
    #[serde(rename = "durationMs")]
    #[sqlx(rename = "durationMs")]
    pub duration_ms: Option<i64>,
    #[serde(rename = "triggeredBy")]
    #[sqlx(rename = "triggeredBy")]
    pub triggered_by: String,
    #[serde(rename = "rawResponse")]
    #[sqlx(rename = "rawResponse")]
    pub raw_response: Option<String>,
    #[serde(rename = "createdAt")]
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CheckinSetting {
    pub id: String,
    pub enabled: bool,
    #[serde(rename = "windowStart")]
    #[sqlx(rename = "windowStart")]
    pub window_start: String,
    #[serde(rename = "windowEnd")]
    #[sqlx(rename = "windowEnd")]
    pub window_end: String,
    #[serde(rename = "retryEnabled")]
    #[sqlx(rename = "retryEnabled")]
    pub retry_enabled: bool,
    #[serde(rename = "maxAttemptsPerDay")]
    #[sqlx(rename = "maxAttemptsPerDay")]
    pub max_attempts_per_day: i32,
    /// 批量/定时签到时，相邻账户之间的最小随机延迟（秒）
    #[serde(rename = "batchDelayMin")]
    #[sqlx(rename = "batchDelayMin")]
    pub batch_delay_min: i32,
    /// 批量/定时签到时，相邻账户之间的最大随机延迟（秒）
    #[serde(rename = "batchDelayMax")]
    #[sqlx(rename = "batchDelayMax")]
    pub batch_delay_max: i32,
    #[serde(rename = "updatedAt")]
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    #[serde(rename = "siteType")]
    pub site_type: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "authType")]
    pub auth_type: String,
    #[serde(rename = "accessToken")]
    pub access_token: Option<String>,
    pub cookie: Option<String>,
    #[serde(rename = "customCheckinUrl")]
    pub custom_checkin_url: Option<String>,
    pub enabled: Option<bool>,
    #[serde(rename = "retryEnabled")]
    pub retry_enabled: Option<bool>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAccountRequest {
    pub name: Option<String>,
    #[serde(rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "accessToken")]
    pub access_token: Option<String>,
    pub cookie: Option<String>,
    #[serde(rename = "customCheckinUrl")]
    pub custom_checkin_url: Option<String>,
    pub enabled: Option<bool>,
    #[serde(rename = "retryEnabled")]
    pub retry_enabled: Option<bool>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: AppUser,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub enabled: Option<bool>,
    #[serde(rename = "windowStart")]
    pub window_start: Option<String>,
    #[serde(rename = "windowEnd")]
    pub window_end: Option<String>,
    #[serde(rename = "retryEnabled")]
    pub retry_enabled: Option<bool>,
    #[serde(rename = "maxAttemptsPerDay")]
    pub max_attempts_per_day: Option<i32>,
    #[serde(rename = "batchDelayMin")]
    pub batch_delay_min: Option<i32>,
    #[serde(rename = "batchDelayMax")]
    pub batch_delay_max: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub note: Option<String>,
}
