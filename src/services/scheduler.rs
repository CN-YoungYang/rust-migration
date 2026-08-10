use crate::{
    db,
    services::checkin::runner::{execute_checkin, skip_reason_for_batch},
};
use chrono::{DateTime, Local, Timelike};
use croner::Cron;
use sqlx::SqlitePool;
use std::{str::FromStr, sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::MissedTickBehavior};

pub async fn start_scheduler(db: SqlitePool) {
    tokio::spawn(run_scheduler(db));
}

async fn run_scheduler(db: SqlitePool) {
    // 防重复触发：用 Mutex 保证同一时刻只有一个定时签到任务在执行。
    let checkin_lock: Arc<Mutex<()>> = Arc::new(Mutex::new(()));

    tracing::info!("Scheduler started");

    let checkin_db = db.clone();
    let checkin_task = async move {
        // 每分钟 tick 一次，命中任一 cron 表达式才触发一轮签到（cron 粒度为分钟）。
        // 每 60s 一次 tick 保证相邻 tick 恒跨分钟，同一分钟内不会重复触发。
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            let db = checkin_db.clone();
            let lock = checkin_lock.clone();
            tokio::spawn(async move {
                let _guard = match lock.try_lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        // 上一轮跨过本分钟触发时该轮被放弃（cron 触发是瞬时点，错过不回补，
                        // 与系统 cron 的语义一致）；但下一轮 60s 后会重新判 cron，默认
                        // */5 计划很快再命中，只有稀疏计划（如每日一次）在长轮占锁时才会漏一次。
                        tracing::warn!("上一轮定时签到仍在执行，跳过本轮以避免重复触发");
                        return;
                    }
                };
                if let Err(e) = check_and_run_scheduled_checkins(&db).await {
                    tracing::error!("Scheduled checkin error: {}", e);
                }
            });
        }
    };

    let cleanup_task = async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10 * 60));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            cleanup_old_runs(&db).await;
        }
    };

    tokio::join!(checkin_task, cleanup_task);
}

async fn cleanup_old_runs(db: &SqlitePool) {
    let keep_latest = match db::get_settings(db).await {
        Ok(s) => s.cleanup_keep_latest.max(0) as usize,
        Err(_) => 500, // fallback
    };
    if let Err(e) = db::cleanup_checkin_data(db, keep_latest, None, false).await {
        tracing::warn!("Run cleanup error: {}", e);
    }
}

/// 判断当前时间是否命中任一 cron 表达式（标准 5 段：分 时 日 月 周，粒度为分钟）。
/// 先把 now 截断到整分（秒/纳秒清零），避免 is_time_matching 校验秒字段导致同一分钟内漏判。
fn cron_now_matches(exprs: &[String], now: DateTime<Local>) -> bool {
    let Some(truncated) = now.with_second(0).and_then(|t| t.with_nanosecond(0)) else {
        return false;
    };
    exprs.iter().any(|expr| {
        Cron::from_str(expr)
            .ok()
            .and_then(|c| c.is_time_matching(&truncated).ok())
            .unwrap_or(false)
    })
}

async fn check_and_run_scheduled_checkins(db: &SqlitePool) -> anyhow::Result<()> {
    let settings = db::get_settings(db).await?;

    if !settings.enabled {
        return Ok(());
    }

    // cron 触发：当前分钟命中任一配置表达式才进入本轮。
    // 每次触发独立成一轮，下方仍从 DB 实时重算今日各账户次数（不做跨触发内存累计）。
    if !cron_now_matches(&settings.schedule_cron, Local::now()) {
        return Ok(());
    }

    // 只查询已启用账户，避免拉取禁用账户再在 Rust 中过滤
    let mut accounts = db::list_enabled_accounts(db).await?;
    let today_local = Local::now().date_naive();

    // 批量查询今日各账户签到次数，避免逐账户 COUNT
    let mut today_counts = db::count_runs_today_for_accounts(db, &[])
        .await
        .unwrap_or_default();

    // 防判定：打乱执行顺序，避免每次按固定顺序签到
    use rand::seq::SliceRandom;
    accounts.shuffle(&mut rand::thread_rng());

    // 串行执行 + 随机间隔：与批量手动签到一致，避免瞬时并发被站点判定为机器人
    let mut executed = 0usize;
    for account in accounts {
        // 跳过今日已签/已禁用/不允许重试的账户（与批量手动签到共用同一判断）
        if let Some(reason) = skip_reason_for_batch(&account, &settings, today_local) {
            tracing::debug!("Skipping account {}: {}", account.id, reason);
            continue;
        }

        // Enforce maxAttemptsPerDay: 使用内存计数器（含本轮已执行的签到）
        let today_runs = today_counts.get(&account.id).copied().unwrap_or(0);
        if today_runs >= settings.max_attempts_per_day.max(1) {
            tracing::debug!(
                "Skipping account {}: {}/{} attempts today",
                account.id,
                today_runs,
                settings.max_attempts_per_day
            );
            continue;
        }

        // 首个账户不延迟，其余账户签到前随机 sleep（按管理员设置）
        // 定时调度专用 scheduledDelayMin/Max，与批量手动签到的 batchDelayMin/Max 解耦。
        if executed > 0 {
            if let Some(secs) = crate::services::checkin::random_delay_secs(
                settings.scheduled_delay_min,
                settings.scheduled_delay_max,
            ) {
                tracing::debug!(
                    "Scheduled checkin: account {} waiting {}s",
                    account.id,
                    secs
                );
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            }
        }

        // 传入 settings 供 execute_checkin 做 TOCTOU 重检查
        match execute_checkin(db, &account.id, "scheduled", Some(&settings)).await {
            Ok(run) => {
                tracing::info!("Scheduled checkin completed for account {}", account.id);
                // 只对真实尝试（success/failed）累加内存计数，与 DB 计数口径一致；
                // already_checked/skipped 不计入每日上限（M6）。
                if db::is_real_attempt(&run.status) {
                    *today_counts.entry(account.id.clone()).or_insert(0) += 1;
                }
            }
            Err(e) => tracing::error!("Scheduled checkin failed for account {}: {}", account.id, e),
        }
        executed += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn local_at(ymd_hms: &str) -> DateTime<Local> {
        let nd = chrono::NaiveDateTime::parse_from_str(ymd_hms, "%Y-%m-%dT%H:%M:%S").unwrap();
        Local.from_local_datetime(&nd).single().unwrap()
    }

    #[test]
    fn cron_matches_at_minute_start() {
        assert!(cron_now_matches(
            &["*/5 2-5 * * *".to_string()],
            local_at("2026-08-10T02:00:00")
        ));
        assert!(cron_now_matches(
            &["*/5 2-5 * * *".to_string()],
            local_at("2026-08-10T04:55:00")
        ));
    }

    #[test]
    fn cron_matches_within_the_minute() {
        // 02:00:37 截断到整分后仍命中，避免秒字段漏判
        assert!(cron_now_matches(
            &["*/5 2-5 * * *".to_string()],
            local_at("2026-08-10T02:00:37")
        ));
    }

    #[test]
    fn cron_not_matching_outside_schedule() {
        assert!(!cron_now_matches(
            &["*/5 2-5 * * *".to_string()],
            local_at("2026-08-10T06:00:00")
        ));
        assert!(!cron_now_matches(
            &["*/5 2-5 * * *".to_string()],
            local_at("2026-08-10T01:59:00")
        ));
    }

    #[test]
    fn cron_any_of_multiple_expressions() {
        let exprs = vec!["0 8 * * *".to_string(), "30 20 * * *".to_string()];
        assert!(cron_now_matches(&exprs, local_at("2026-08-10T08:00:00")));
        assert!(cron_now_matches(&exprs, local_at("2026-08-10T20:30:00")));
        assert!(!cron_now_matches(&exprs, local_at("2026-08-10T08:30:00")));
    }

    #[test]
    fn cron_daily_hourly_and_step() {
        assert!(cron_now_matches(
            &["0 3 * * *".to_string()],
            local_at("2026-08-10T03:00:00")
        ));
        assert!(!cron_now_matches(
            &["0 3 * * *".to_string()],
            local_at("2026-08-10T03:30:00")
        ));
        assert!(cron_now_matches(
            &["*/30 * * * *".to_string()],
            local_at("2026-08-10T09:00:00")
        ));
        assert!(cron_now_matches(
            &["*/30 * * * *".to_string()],
            local_at("2026-08-10T09:30:00")
        ));
        assert!(!cron_now_matches(
            &["*/30 * * * *".to_string()],
            local_at("2026-08-10T09:15:00")
        ));
    }

    #[test]
    fn invalid_cron_expression_is_ignored() {
        assert!(!cron_now_matches(
            &["not-a-cron".to_string()],
            local_at("2026-08-10T08:00:00")
        ));
    }
}
