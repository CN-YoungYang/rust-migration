use axum::{
    extract::State,
    Json,
};
use std::sync::Arc;
use crate::{
    AppState,
    models::{CheckinSetting, UpdateSettingsRequest},
    error::Result,
    db,
};

pub async fn get(State(state): State<Arc<AppState>>) -> Result<Json<CheckinSetting>> {
    let settings = db::get_settings(&state.db).await?;
    Ok(Json(settings))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<CheckinSetting>> {
    let settings = db::update_settings(
        &state.db,
        payload.enabled,
        payload.window_start.as_deref(),
        payload.window_end.as_deref(),
        payload.retry_enabled,
        payload.max_attempts_per_day,
    ).await?;
    
    Ok(Json(settings))
}
