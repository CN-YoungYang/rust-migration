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
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsRequest {
    pub enabled: Option<bool>,
    #[serde(alias = "window_start")]
    pub window_start: Option<String>,
    #[serde(alias = "window_end")]
    pub window_end: Option<String>,
    #[serde(alias = "retry_enabled")]
    pub retry_enabled: Option<bool>,
    #[serde(alias = "max_attempts_per_day")]
    pub max_attempts_per_day: Option<i32>,
    #[serde(alias = "batch_delay_min")]
    pub batch_delay_min: Option<i32>,
    #[serde(alias = "batch_delay_max")]
    pub batch_delay_max: Option<i32>,
    #[serde(alias = "cleanup_keep_latest")]
    pub cleanup_keep_latest: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::UpdateSettingsRequest;

    #[test]
    fn settings_update_request_accepts_frontend_camel_case_fields() {
        let req: UpdateSettingsRequest = serde_json::from_str(
            r#"{
                "enabled": true,
                "windowStart": "03:00",
                "windowEnd": "06:30",
                "retryEnabled": false,
                "maxAttemptsPerDay": 4,
                "batchDelayMin": 1,
                "batchDelayMax": 9,
                "cleanupKeepLatest": 321
            }"#,
        )
        .unwrap();

        assert_eq!(req.enabled, Some(true));
        assert_eq!(req.window_start, Some("03:00".to_string()));
        assert_eq!(req.window_end, Some("06:30".to_string()));
        assert_eq!(req.retry_enabled, Some(false));
        assert_eq!(req.max_attempts_per_day, Some(4));
        assert_eq!(req.batch_delay_min, Some(1));
        assert_eq!(req.batch_delay_max, Some(9));
        assert_eq!(req.cleanup_keep_latest, Some(321));
    }

    #[test]
    fn settings_update_request_keeps_snake_case_compatibility() {
        let req: UpdateSettingsRequest = serde_json::from_str(
            r#"{
                "window_start": "04:00",
                "window_end": "07:30",
                "retry_enabled": true,
                "max_attempts_per_day": 5,
                "batch_delay_min": 2,
                "batch_delay_max": 8,
                "cleanup_keep_latest": 654
            }"#,
        )
        .unwrap();

        assert_eq!(req.window_start, Some("04:00".to_string()));
        assert_eq!(req.window_end, Some("07:30".to_string()));
        assert_eq!(req.retry_enabled, Some(true));
        assert_eq!(req.max_attempts_per_day, Some(5));
        assert_eq!(req.batch_delay_min, Some(2));
        assert_eq!(req.batch_delay_max, Some(8));
        assert_eq!(req.cleanup_keep_latest, Some(654));
    }
}
