use axum::Json;
use serde_json::{json, Value};

pub async fn health() -> Json<Value> {
    crate::routes::data(json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
