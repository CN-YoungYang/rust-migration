use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, Method};
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, services::ServeDir, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth_middleware;
mod crypto;
mod db;
mod error;
mod models;
mod routes;
mod security;
mod services;

use crate::auth_middleware::{admin_middleware, auth_middleware};
use crate::services::scheduler::start_scheduler;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/ai-hub.db".to_string());
    let connect_options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .pragma("cache_size", "-2000") // 2MB cache
        .pragma("temp_store", "memory");

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await?;
    sqlx::query("PRAGMA max_page_count = 1073741823")
        .execute(&db)
        .await?;

    // Run migrations
    let migration_sql = include_str!("../migrations/20260611_init.sql");
    sqlx::query(migration_sql).execute(&db).await?;

    initialize_admin(&db).await?;
    db::ensure_setting_columns(&db).await?;

    let state = Arc::new(AppState { db: db.clone() });

    start_scheduler(db.clone()).await;
    auth_middleware::start_session_cleanup_task(db.clone());

    // Public routes
    let public_routes = Router::new()
        .route("/api/health", get(routes::health::health))
        .route("/api/server-time", get(routes::server_time::server_time))
        .route("/api/auth/login", post(routes::auth::login))
        .route("/api/auth/logout", post(routes::auth::logout));

    // Protected routes
    let protected_routes = Router::new()
        .route("/api/auth/me", get(routes::auth::me))
        .route(
            "/api/accounts",
            get(routes::accounts::list).post(routes::accounts::create),
        )
        .route(
            "/api/accounts/:id",
            get(routes::accounts::get)
                .put(routes::accounts::update)
                .delete(routes::accounts::delete),
        )
        .route(
            "/api/accounts/:id/refresh-balance",
            post(routes::accounts::refresh_balance),
        )
        .route(
            "/api/accounts/export",
            get(routes::import_export::export_accounts),
        )
        .route(
            "/api/accounts/import",
            post(routes::import_export::import_accounts),
        )
        .route(
            "/api/settings",
            get(routes::settings::get).put(routes::settings::update),
        )
        .route(
            "/api/checkin-runs",
            get(routes::checkin_runs::list).post(routes::checkin_runs::execute),
        )
        .route(
            "/api/checkin-runs/batch",
            post(routes::checkin_runs::execute_batch),
        )
        .route(
            "/api/checkin-runs/cleanup",
            post(routes::checkin_runs::cleanup_runs),
        )
        .route("/api/statistics", get(routes::statistics::get_statistics))
        .route(
            "/api/notifications",
            get(routes::notifications::list_notifications)
                .post(routes::notifications::create_notification),
        )
        .route(
            "/api/notifications/:id",
            get(routes::notifications::get_notification)
                .put(routes::notifications::update_notification)
                .delete(routes::notifications::delete_notification),
        )
        .route(
            "/api/notifications/:id/test",
            post(routes::notifications::test_notification),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Admin routes
    let admin_routes = Router::new()
        .route(
            "/api/admin/users",
            get(routes::admin::list_users).post(routes::admin::create_user),
        )
        .route(
            "/api/admin/users/:id",
            get(routes::admin::get_user)
                .put(routes::admin::update_user)
                .delete(routes::admin::delete_user),
        )
        .layer(middleware::from_fn(admin_middleware))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(admin_routes)
        .layer(CompressionLayer::new())
        .layer(DefaultBodyLimit::max(request_body_limit_bytes()))
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer())
        .fallback_service(ServeDir::new("public"))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", listener.local_addr()?);
    tracing::info!("Memory optimized for 1C1G server");
    axum::serve(listener, app).await?;

    Ok(())
}

fn request_body_limit_bytes() -> usize {
    const DEFAULT_LIMIT: usize = 5 * 1024 * 1024;
    std::env::var("REQUEST_BODY_LIMIT_MB")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|mb| (1..=50).contains(mb))
        .map(|mb| mb * 1024 * 1024)
        .unwrap_or(DEFAULT_LIMIT)
}

fn cors_layer() -> CorsLayer {
    let origins = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());
    let mut layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("x-csrf-token"),
            HeaderName::from_static("accept"),
        ])
        .allow_credentials(true);

    for origin in origins
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
    {
        if origin == "*" {
            tracing::warn!("CORS wildcard (*) cannot be used with credentials. Skipping.");
            continue;
        }
        if let Ok(header_value) = HeaderValue::from_str(origin) {
            layer = layer.allow_origin(header_value);
        } else {
            tracing::warn!("Ignoring invalid CORS origin: {}", origin);
        }
    }

    layer
}
async fn initialize_admin(db: &SqlitePool) -> anyhow::Result<()> {
    let admin_username = std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());

    tracing::info!("Initializing admin user: {}", admin_username);

    let existing = db::find_user_by_username(db, &admin_username).await?;

    if existing.is_none() {
        let admin_password = std::env::var("ADMIN_PASSWORD")
            .map_err(|_| anyhow::anyhow!("ADMIN_PASSWORD 环境变量必须设置"))?;
        if admin_password.len() < 8 {
            anyhow::bail!("ADMIN_PASSWORD 至少需要 8 个字符");
        }
        let password_hash = crypto::hash_password(&admin_password)?;
        db::create_user(
            db,
            &admin_username,
            &password_hash,
            "SUPER_ADMIN",
            true,
            None,
        )
        .await?;
        tracing::info!("Created SUPER_ADMIN user: {}", admin_username);
    } else {
        tracing::info!("Admin user already exists: {}", admin_username);
    }

    Ok(())
}
