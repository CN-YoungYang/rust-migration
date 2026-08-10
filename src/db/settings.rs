use super::types::UpdateSettingsRequest;
use crate::error::Result;
use crate::models::CheckinSetting;
use chrono::Utc;
use croner::Cron;
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// 默认调度计划：每日 02:00–05:59 每 5 分钟触发（近似旧窗口 02:00–05:00，保留窗口内重试语义）。
/// 新增/重置 scheduleCron 时统一走该常量，避免散落硬编码字符串。
const DEFAULT_SCHEDULE_CRON: &str = "*/5 2-5 * * *";

/// Settings memory cache: avoids querying DB on every request (single-row table, rarely changes)
static SETTINGS_CACHE: std::sync::OnceLock<RwLock<Option<(CheckinSetting, Instant)>>> =
    std::sync::OnceLock::new();
const SETTINGS_CACHE_TTL: Duration = Duration::from_secs(30);

fn settings_cache() -> &'static RwLock<Option<(CheckinSetting, Instant)>> {
    SETTINGS_CACHE.get_or_init(|| RwLock::new(None))
}

/// 旧库升级（无 scheduleCron 列）时，把历史 windowStart/windowEnd 迁移成 cron 表达式列表。
/// 默认窗口 02:00–05:00 返回 None（保持 ALTER 默认计划即可）；非默认窗口尽力转换，
/// 返回 JSON 数组字符串，无法解析时返回 None 由调用方告警并使用默认计划。
/// 旧窗口是"每 5 分钟 tick + 落在窗口内才跑"，分钟端点的精确性无法用 5 段 cron 表达，
/// 此处按小时粒度近似（如 08:00–09:30 → */5 8-9 * * *），越界触发由签到侧跳过逻辑兜底。
async fn window_to_cron_on_upgrade(db: &SqlitePool) -> Option<String> {
    let (start, end): (String, String) =
        sqlx::query_as("SELECT windowStart, windowEnd FROM CheckinSetting WHERE id = 'global'")
            .fetch_optional(db)
            .await
            .ok()?
            .unwrap_or(("02:00".into(), "05:00".into()));

    if start == "02:00" && end == "05:00" {
        return None;
    }

    let hour = |t: &str| -> Option<u32> { t.trim().split(':').next()?.trim().parse::<u32>().ok() };
    let sh = hour(&start)?;
    let eh = hour(&end)?;
    if sh > 23 || eh > 23 {
        return None;
    }

    let exprs = if sh <= eh {
        vec![format!("*/5 {sh}-{eh} * * *")]
    } else {
        // 跨午夜窗口拆成两条表达式，如 22:00–02:00 → */5 22-23 * * * 与 */5 0-2 * * *
        vec![format!("*/5 {sh}-23 * * *"), format!("*/5 0-{eh} * * *")]
    };
    serde_json::to_string(&exprs).ok()
}

/// Idempotent migration for old databases: batch checkin random delay columns introduced in v2.2.2.
/// Existing databases need these columns added. SQLite doesn't support ADD COLUMN IF NOT EXISTS,
/// so we try + ignore "duplicate column" errors.
pub async fn ensure_setting_columns(db: &SqlitePool) -> Result<()> {
    for (col, default_value) in [
        ("batchDelayMin", 3),
        ("batchDelayMax", 10),
        ("scheduledDelayMin", 3),
        ("scheduledDelayMax", 10),
    ] {
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
    // scheduleCron column (cron trigger schedule as a JSON array of 5-field expressions,
    // introduced in v2.6.0 to replace windowStart/windowEnd which are kept for old-DB compat).
    // 记录该列是否为本次新加：若旧库升级（此前只有窗口），需要把非默认窗口迁移成 cron，
    // 否则升级后调度会被默认计划静默替换，行为无声改变。
    let schedule_cron_added = match sqlx::query(format!(
        "ALTER TABLE CheckinSetting ADD COLUMN scheduleCron TEXT NOT NULL DEFAULT '[\"{DEFAULT_SCHEDULE_CRON}\"]'"
    ).as_str())
    .execute(db)
    .await
    {
        Ok(_) => true,
        Err(e) => {
            let msg = e.to_string();
            if !msg.contains("duplicate column") {
                return Err(e.into());
            }
            false
        }
    };
    if schedule_cron_added {
        if let Some(cron) = window_to_cron_on_upgrade(db).await {
            tracing::info!("已把旧签到窗口迁移为 cron 计划: {cron}");
            if let Err(e) =
                sqlx::query("UPDATE CheckinSetting SET scheduleCron = ? WHERE id = 'global'")
                    .bind(cron)
                    .execute(db)
                    .await
            {
                tracing::warn!("窗口迁移写回 scheduleCron 失败: {}", e);
            }
        } else {
            tracing::warn!(
                "旧签到窗口无法转换，已使用默认 cron 计划 '{}'，请在设置面板重新配置",
                DEFAULT_SCHEDULE_CRON
            );
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
        "SELECT id, enabled, scheduleCron, retryEnabled, maxAttemptsPerDay, \
         batchDelayMin, batchDelayMax, scheduledDelayMin, scheduledDelayMax, \
         cleanupKeepLatest, updatedAt \
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
        if s.scheduled_delay_min < 0 {
            s.scheduled_delay_min = 0;
            needs_update = true;
        }
        if s.scheduled_delay_max < s.scheduled_delay_min {
            s.scheduled_delay_max = s.scheduled_delay_min;
            needs_update = true;
        }
        if s.cleanup_keep_latest < 0 {
            s.cleanup_keep_latest = 500;
            needs_update = true;
        }
        // 调度计划自愈：过滤非法条目（不可解析、或非标准 5 段——6/7 段带秒字段会永不触发），
        // 过滤后为空则回写默认计划，避免库里残留非法 cron 静默禁用整个调度。
        let valid: Vec<String> = s
            .schedule_cron
            .iter()
            .filter(|e| e.split_whitespace().count() == 5 && Cron::from_str(e).is_ok())
            .cloned()
            .collect();
        if valid.len() != s.schedule_cron.len() {
            tracing::warn!(
                "调度计划含 {} 条非法 cron 表达式，已忽略；结果为空时使用默认计划 '{}'",
                s.schedule_cron.len() - valid.len(),
                DEFAULT_SCHEDULE_CRON
            );
            s.schedule_cron = if valid.is_empty() {
                vec![DEFAULT_SCHEDULE_CRON.to_string()]
            } else {
                valid
            };
            needs_update = true;
        }
        if needs_update {
            if let Err(e) = sqlx::query(
                "UPDATE CheckinSetting SET batchDelayMin = ?, batchDelayMax = ?, \
                 scheduledDelayMin = ?, scheduledDelayMax = ?, cleanupKeepLatest = ?, \
                 scheduleCron = ? \
                 WHERE id = 'global'",
            )
            .bind(s.batch_delay_min)
            .bind(s.batch_delay_max)
            .bind(s.scheduled_delay_min)
            .bind(s.scheduled_delay_max)
            .bind(s.cleanup_keep_latest)
            .bind(serde_json::to_string(&s.schedule_cron).unwrap_or_default())
            .execute(db)
            .await
            {
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
            "INSERT INTO CheckinSetting (id, enabled, scheduleCron, retryEnabled, maxAttemptsPerDay, batchDelayMin, batchDelayMax, scheduledDelayMin, scheduledDelayMax, cleanupKeepLatest, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("global")
        .bind(false)
        // scheduleCron 默认：每日 02:00–05:59 每 5 分钟触发（近似旧窗口 02:00–05:00，保留窗口内重试语义）
        .bind(serde_json::to_string(&vec![DEFAULT_SCHEDULE_CRON.to_string()]).unwrap_or_default())
        .bind(true)
        .bind(3)
        .bind(3)   // batchDelayMin default 3 seconds
        .bind(10)  // batchDelayMax default 10 seconds
        .bind(3)   // scheduledDelayMin default 3 seconds
        .bind(10)  // scheduledDelayMax default 10 seconds
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

    // 调度计划以 JSON 数组字符串落库（列类型 TEXT）。
    let schedule_cron_json = match &req.schedule_cron {
        Some(list) => serde_json::to_string(list).unwrap_or_else(|_| "[]".to_string()),
        None => serde_json::to_string(&current.schedule_cron).unwrap_or_default(),
    };

    let settings = sqlx::query_as::<_, CheckinSetting>(
        "UPDATE CheckinSetting SET enabled = ?, scheduleCron = ?, retryEnabled = ?, maxAttemptsPerDay = ?, batchDelayMin = ?, batchDelayMax = ?, scheduledDelayMin = ?, scheduledDelayMax = ?, cleanupKeepLatest = ?, updatedAt = ? WHERE id = 'global' RETURNING *"
    )
    .bind(req.enabled.unwrap_or(current.enabled))
    .bind(schedule_cron_json)
    .bind(req.retry_enabled.unwrap_or(current.retry_enabled))
    .bind(req.max_attempts_per_day.unwrap_or(current.max_attempts_per_day))
    .bind(req.batch_delay_min.unwrap_or(current.batch_delay_min))
    .bind(req.batch_delay_max.unwrap_or(current.batch_delay_max))
    .bind(req.scheduled_delay_min.unwrap_or(current.scheduled_delay_min))
    .bind(req.scheduled_delay_max.unwrap_or(current.scheduled_delay_max))
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
                scheduled_delay_min: Some(0),
                scheduled_delay_max: Some(0),
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
        assert_eq!(settings.scheduled_delay_min, 0);
        assert_eq!(settings.scheduled_delay_max, 0);
        assert_eq!(settings.cleanup_keep_latest, 0);
    }

    #[tokio::test]
    async fn get_settings_rewrites_invalid_scheduled_delay_range() {
        // 旧库可能因手动改库出现 max<min 或负值，get_settings 的回写兜底应修正。
        let pool = setup_db().await;
        sqlx::query(
            "UPDATE CheckinSetting SET scheduledDelayMin = -1, scheduledDelayMax = -5 \
             WHERE id = 'global'",
        )
        .execute(&pool)
        .await
        .unwrap();
        clear_settings_cache_for_test();
        let settings = get_settings(&pool).await.unwrap();

        assert_eq!(settings.scheduled_delay_min, 0);
        assert_eq!(settings.scheduled_delay_max, 0);
    }

    #[tokio::test]
    async fn window_to_cron_default_window_keeps_default() {
        let pool = setup_db().await;
        assert!(window_to_cron_on_upgrade(&pool).await.is_none());
    }

    #[tokio::test]
    async fn window_to_cron_converts_custom_window() {
        let pool = setup_db().await;
        sqlx::query(
            "UPDATE CheckinSetting SET windowStart = '08:00', windowEnd = '09:30' \
             WHERE id = 'global'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let cron = window_to_cron_on_upgrade(&pool).await.unwrap();
        let list: Vec<String> = serde_json::from_str(&cron).unwrap();
        assert_eq!(list, vec!["*/5 8-9 * * *"]);
    }

    #[tokio::test]
    async fn window_to_cron_cross_midnight_splits_two_exprs() {
        let pool = setup_db().await;
        sqlx::query(
            "UPDATE CheckinSetting SET windowStart = '22:00', windowEnd = '02:00' \
             WHERE id = 'global'",
        )
        .execute(&pool)
        .await
        .unwrap();
        let cron = window_to_cron_on_upgrade(&pool).await.unwrap();
        let list: Vec<String> = serde_json::from_str(&cron).unwrap();
        assert_eq!(list, vec!["*/5 22-23 * * *", "*/5 0-2 * * *"]);
    }

    #[tokio::test]
    async fn get_settings_heals_partially_invalid_schedule_cron() {
        let pool = setup_db().await;
        sqlx::query(
            "UPDATE CheckinSetting SET scheduleCron = '[\"not-a-cron\", \"0 3 * * *\"]' \
             WHERE id = 'global'",
        )
        .execute(&pool)
        .await
        .unwrap();
        clear_settings_cache_for_test();
        let settings = get_settings(&pool).await.unwrap();
        // 非法条目被过滤，合法条目保留
        assert_eq!(settings.schedule_cron, vec!["0 3 * * *".to_string()]);
    }

    #[tokio::test]
    async fn get_settings_heals_all_invalid_schedule_cron_to_default() {
        let pool = setup_db().await;
        // 含 6 段（带秒）与不可解析条目：全部过滤后应回写默认计划
        sqlx::query(
            "UPDATE CheckinSetting SET scheduleCron = '[\"not-a-cron\", \"30 * * * * *\"]' \
             WHERE id = 'global'",
        )
        .execute(&pool)
        .await
        .unwrap();
        clear_settings_cache_for_test();
        let settings = get_settings(&pool).await.unwrap();
        assert_eq!(
            settings.schedule_cron,
            vec![DEFAULT_SCHEDULE_CRON.to_string()]
        );
    }
}
