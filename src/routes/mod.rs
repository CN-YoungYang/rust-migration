pub mod accounts;
pub mod admin;
pub mod auth;
pub mod checkin_runs;
pub mod health;
pub mod import_export;
pub mod notifications;
pub mod server_time;
pub mod settings;
pub mod statistics;

use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

pub fn data<T: Serialize>(value: T) -> Json<Value> {
    Json(json!({ "data": value }))
}
