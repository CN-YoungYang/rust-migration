use axum::Json;
use serde_json::{json, Value};

pub async fn server_time() -> Json<Value> {
    crate::routes::data(json!({
        "serverTime": chrono::Utc::now().to_rfc3339()
    }))
}
