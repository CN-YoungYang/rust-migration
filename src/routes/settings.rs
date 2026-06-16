use axum::{
    extract::{State, Extension},
    Json,
};
use std::sync::Arc;
use crate::{
    AppState,
    models::{AppUser, CheckinSetting, UpdateSettingsRequest},
    error::{Result, AppError},
    db,
};

fn require_admin(user: &AppUser) -> Result<()> {
    if user.role != "ADMIN" && user.role != "SUPER_ADMIN" {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
) -> Result<Json<CheckinSetting>> {
    require_admin(&user)?;
    let settings = db::get_settings(&state.db).await?;
    Ok(Json(settings))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<CheckinSetting>> {
    require_admin(&user)?;
    if let Some(ref start) = payload.window_start {
        if start.parse::<chrono::NaiveTime>().is_err() {
            return Err(AppError::Validation("windowStart must be in HH:MM format".into()));
        }
    }
    if let Some(ref end) = payload.window_end {
        if end.parse::<chrono::NaiveTime>().is_err() {
            return Err(AppError::Validation("windowEnd must be in HH:MM format".into()));
        }
    }
    if let Some(max) = payload.max_attempts_per_day {
        if max < 1 || max > 100 {
            return Err(AppError::Validation("maxAttemptsPerDay must be between 1 and 100".into()));
        }
    }

    // 批量/定时签到随机延迟范围校验（秒）。允许 min=0 且 max=0 表示不延迟。
    // 约束：0 <= min <= max <= 600（10 分钟封顶，避免单次签到耗时过长）。
    if let (Some(min), Some(max)) = (payload.batch_delay_min, payload.batch_delay_max) {
        if min < 0 || max < 0 || min > max || max > 600 {
            return Err(AppError::Validation(
                "batchDelayMin/Max 必须满足 0 <= min <= max <= 600（秒）".into(),
            ));
        }
    } else if payload.batch_delay_min.is_some() || payload.batch_delay_max.is_some() {
        return Err(AppError::Validation("batchDelayMin 和 batchDelayMax 必须同时提供".into()));
    }

    let settings = db::update_settings(
        &state.db,
        payload.enabled,
        payload.window_start.as_deref(),
        payload.window_end.as_deref(),
        payload.retry_enabled,
        payload.max_attempts_per_day,
        payload.batch_delay_min,
        payload.batch_delay_max,
    ).await?;

    Ok(Json(settings))
}
