use super::types::UpdateSettingsRequest;
use crate::error::Result;
use crate::models::CheckinSetting;
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Settings memory cache: avoids querying DB on every request (single-row table, rarely changes)
static SETTINGS_CACHE: std::sync::OnceLock<RwLock<Option<(CheckinSetting, Instant)>>> =
    std::sync::OnceLock::new();
const SETTINGS_CACHE_TTL: Duration = Duration::from_secs(30);

fn settings_cache() -> &'static RwLock<Option<(CheckinSetting, Instant)>> {
    SETTINGS_CACHE.get_or_init(|| RwLock::new(None))
}

/// Idempotent migration for old databases: batch checkin random delay columns introduced in v2.2.2.
/// Existing databases need these columns added. SQLite doesn't support ADD COLUMN IF NOT EXISTS,
/// so we try + ignore "duplicate column" errors.
pub async fn ensure_setting_columns(db: &SqlitePool) -> Result<()> {
    for (col, default_value) in [("batchDelayMin", 3), ("batchDelayMax", 10)] {
        let sql = format!(
            "ALTER TABLE CheckinSetting ADD COLUMN {} INTEGER NOT NULL DEFAULT {}",
            col, default_value
        );
        if let Err(e) = sqlx::query(&sql).execute(db).await {
            let msg = e.to_string();
            if !msg.contains("duplicate column") {
                return Err(e.into());
            }
        }
    }
    // cleanupKeepLatest column (scheduled cleanup retention count, introduced in v2.3.3)
    if let Err(e) = sqlx::query(
        "ALTER TABLE CheckinSetting ADD COLUMN cleanupKeepLatest INTEGER NOT NULL DEFAULT 500",
    )
    .execute(db)
    .await
    {
        let msg = e.to_string();
        if !msg.contains("duplicate column") {
            return Err(e.into());
        }
    }
    // CheckinAccount.note column (introduced in v2.3.2)
    if let Err(e) = sqlx::query("ALTER TABLE CheckinAccount ADD COLUMN note TEXT")
        .execute(db)
        .await
    {
        let msg = e.to_string();
        if !msg.contains("duplicate column") {
            return Err(e.into());
        }
    }

    // Indexes needed for filtering functionality
    let indexes = [
        "CREATE INDEX IF NOT EXISTS idx_checkin_account_site_type ON CheckinAccount(siteType)",
        "CREATE INDEX IF NOT EXISTS idx_checkin_account_last_status ON CheckinAccount(lastStatus)",
        "CREATE INDEX IF NOT EXISTS idx_checkin_run_triggered_by ON CheckinRun(triggeredBy)",
        "CREATE INDEX IF NOT EXISTS idx_checkin_run_status_created ON CheckinRun(status, createdAt)",
    ];
    for idx_sql in indexes {
        if let Err(e) = sqlx::query(idx_sql).execute(db).await {
            let msg = e.to_string();
            // Ignore already-exists errors
            if !msg.contains("already exists") {
                tracing::warn!("Failed to create index: {} - {}", idx_sql, e);
            }
        }
    }

    Ok(())
}

/// Get global settings (with caching)
pub async fn get_settings(db: &SqlitePool) -> Result<CheckinSetting> {
    // Check cache
    {
        let cache = settings_cache().read().unwrap_or_else(|e| e.into_inner());
        if let Some((settings, cached_at)) = cache.as_ref() {
            if cached_at.elapsed() < SETTINGS_CACHE_TTL {
                return Ok(settings.clone());
            }
        }
    }

    let settings = sqlx::query_as::<_, CheckinSetting>(
        "SELECT id, enabled, windowStart, windowEnd, retryEnabled, maxAttemptsPerDay, \
         batchDelayMin, batchDelayMax, cleanupKeepLatest, updatedAt \
         FROM CheckinSetting WHERE id = 'global'",
    )
    .fetch_optional(db)
    .await?;

    if let Some(s) = settings {
        let mut s = s;
        let mut needs_update = false;
        if s.batch_delay_min < 0 {
            s.batch_delay_min = 0;
            needs_update = true;
        }
        if s.batch_delay_max < s.batch_delay_min {
            s.batch_delay_max = s.batch_delay_min;
            needs_update = true;
        }
        if s.cleanup_keep_latest < 0 {
            s.cleanup_keep_latest = 500;
            needs_update = true;
        }
        if needs_update {
            if let Err(e) = sqlx::query(
                "UPDATE CheckinSetting SET batchDelayMin = ?, batchDelayMax = ?, cleanupKeepLatest = ? WHERE id = 'global'"
            )
            .bind(s.batch_delay_min)
            .bind(s.batch_delay_max)
            .bind(s.cleanup_keep_latest)
            .execute(db)
            .await {
                tracing::warn!("Failed to write back settings defaults: {}", e);
            }
        }
        // Write to cache
        {
            let mut cache = settings_cache().write().unwrap_or_else(|e| e.into_inner());
            *cache = Some((s.clone(), Instant::now()));
        }
        Ok(s)
    } else {
        // Create default settings
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinSetting (id, enabled, windowStart, windowEnd, retryEnabled, maxAttemptsPerDay, batchDelayMin, batchDelayMax, cleanupKeepLatest, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("global")
        .bind(false)
        .bind("02:00")
        .bind("05:00")
        .bind(true)
        .bind(3)
        .bind(3)   // batchDelayMin default 3 seconds
        .bind(10)  // batchDelayMax default 10 seconds
        .bind(500) // cleanupKeepLatest default 500 records
        .bind(now)
        .execute(db)
        .await?;

        Box::pin(get_settings(db)).await
    }
}

/// Update global settings
pub async fn update_settings(
    db: &SqlitePool,
    req: &UpdateSettingsRequest,
) -> Result<CheckinSetting> {
    let now = Utc::now();
    let current = Box::pin(get_settings(db)).await?;

    let settings = sqlx::query_as::<_, CheckinSetting>(
        "UPDATE CheckinSetting SET enabled = ?, windowStart = ?, windowEnd = ?, retryEnabled = ?, maxAttemptsPerDay = ?, batchDelayMin = ?, batchDelayMax = ?, cleanupKeepLatest = ?, updatedAt = ? WHERE id = 'global' RETURNING *"
    )
    .bind(req.enabled.unwrap_or(current.enabled))
    .bind(req.window_start.as_ref().unwrap_or(&current.window_start))
    .bind(req.window_end.as_ref().unwrap_or(&current.window_end))
    .bind(req.retry_enabled.unwrap_or(current.retry_enabled))
    .bind(req.max_attempts_per_day.unwrap_or(current.max_attempts_per_day))
    .bind(req.batch_delay_min.unwrap_or(current.batch_delay_min))
    .bind(req.batch_delay_max.unwrap_or(current.batch_delay_max))
    .bind(req.cleanup_keep_latest.unwrap_or(current.cleanup_keep_latest))
    .bind(now)
    .fetch_one(db)
    .await?;

    // Update cache
    {
        let mut cache = settings_cache()
            .write()
            .map_err(|_| crate::error::AppError::Internal("设置缓存锁已损坏".into()))?;
        *cache = Some((settings.clone(), Instant::now()));
    }

    Ok(settings)
}

#[cfg(test)]
fn clear_settings_cache_for_test() {
    if let Some(cache) = SETTINGS_CACHE.get() {
        let mut cache = cache.write().unwrap_or_else(|e| e.into_inner());
        *cache = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::UpdateSettingsRequest;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_db() -> SqlitePool {
        clear_settings_cache_for_test();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(include_str!("../../migrations/20260611_init.sql"))
            .execute(&pool)
            .await
            .unwrap();
        ensure_setting_columns(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn get_settings_preserves_explicit_zero_delay_and_cleanup_retention() {
        let pool = setup_db().await;
        update_settings(
            &pool,
            &UpdateSettingsRequest {
                batch_delay_min: Some(0),
                batch_delay_max: Some(0),
                cleanup_keep_latest: Some(0),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        clear_settings_cache_for_test();
        let settings = get_settings(&pool).await.unwrap();

        assert_eq!(settings.batch_delay_min, 0);
        assert_eq!(settings.batch_delay_max, 0);
        assert_eq!(settings.cleanup_keep_latest, 0);
    }
}
