use axum::{
    extract::State,
    Json,
};
use std::sync::Arc;
use serde::{Deserialize};
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

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<Vec<CheckinRun>>> {
    let runs = db::list_runs(&state.db, 100).await?;
    Ok(Json(runs))
}

pub async fn execute(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExecuteCheckinRequest>,
) -> Result<Json<CheckinRun>> {
    let run = execute_checkin(&state.db, &payload.account_id, "manual").await?;
    Ok(Json(run))
}
