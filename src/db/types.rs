use serde::Deserialize;

/// Account list filter parameters
#[derive(Debug, Clone, Default)]
pub struct AccountFilter {
    pub owner_id: Option<String>,
    pub site_type: Option<String>,
    pub enabled: Option<bool>,
    pub last_status: Option<String>,
    pub keyword: Option<String>,
    pub limit: i32,
    pub offset: i32,
}

/// Check-in run list filter parameters
#[derive(Debug, Clone, Default)]
pub struct RunFilter {
    pub owner_id: Option<String>,
    pub account_id: Option<String>,
    pub status: Option<String>,
    pub triggered_by: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: i32,
    pub offset: i32,
}

/// Account creation request
#[derive(Debug, Clone)]
pub struct CreateAccountRequest {
    pub name: String,
    pub site_type: String,
    pub base_url: String,
    pub user_id: Option<String>,
    pub auth_type: String,
    pub access_token_enc: Option<String>,
    pub cookie_enc: Option<String>,
    pub custom_checkin_url: Option<String>,
    pub enabled: bool,
    pub retry_enabled: bool,
    pub owner_id: String,
    pub note: Option<String>,
}

/// Account update request
#[derive(Debug, Clone, Default)]
pub struct UpdateAccountRequest {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub user_id: Option<Option<String>>,
    pub access_token_enc: Option<Option<String>>,
    pub cookie_enc: Option<Option<String>>,
    pub custom_checkin_url: Option<Option<String>>,
    pub enabled: Option<bool>,
    pub retry_enabled: Option<bool>,
    pub note: Option<Option<String>>,
}

/// Settings update request
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdateSettingsRequest {
    pub enabled: Option<bool>,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub retry_enabled: Option<bool>,
    pub max_attempts_per_day: Option<i32>,
    pub batch_delay_min: Option<i32>,
    pub batch_delay_max: Option<i32>,
    pub cleanup_keep_latest: Option<i32>,
}
