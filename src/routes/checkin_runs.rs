use axum::{
    extract::{State, Extension},
    Json,
};
use std::sync::Arc;
use serde::{Deserialize};
use serde_json::{json, Value};
use crate::{
    AppState,
    models::CheckinRun,
    error::Result,
    db,
    services::checkin::runner::execute_checkin,
};

#[derive(Debug, Deserialize)]
pub struct ExecuteCheckinRequest {
    pub account_id: String,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
) -> Result<Json<Vec<CheckinRun>>> {
    let runs = if user.role == "ADMIN" || user.role == "SUPER_ADMIN" {
        db::list_runs(&state.db, 100).await?
    } else {
        db::list_runs_by_user(&state.db, &user.id, 100).await?
    };
    Ok(Json(runs))
}

pub async fn execute(
    State(state): State<Arc<AppState>>, Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<ExecuteCheckinRequest>,
) -> Result<Json<CheckinRun>> {
    let account = db::find_account_by_id(&state.db, &payload.account_id).await?.ok_or(crate::error::AppError::NotFound)?;
    if user.role != "ADMIN" && user.role != "SUPER_ADMIN" && account.owner_id.as_ref() != Some(&user.id) {
        return Err(crate::error::AppError::Forbidden);
    }
    let run = execute_checkin(&state.db, &payload.account_id, "manual").await?;
    Ok(Json(run))
}

pub async fn cleanup_runs(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<Value>> {
    let keep_latest = payload["keepLatest"].as_i64().unwrap_or(100) as usize;
    
    let deleted_count = if user.role == "ADMIN" || user.role == "SUPER_ADMIN" {
        db::cleanup_checkin_runs(&state.db, keep_latest).await?
    } else {
        db::cleanup_checkin_runs_by_user(&state.db, &user.id, keep_latest).await?
    };
    
    Ok(Json(json!({
        "deletedCount": deleted_count,
        "keepLatest": keep_latest
    })))
}
