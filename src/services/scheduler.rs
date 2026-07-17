use crate::{
    db,
    services::checkin::runner::{execute_checkin, skip_reason_for_batch},
};
use chrono::{Local, NaiveTime};
use sqlx::SqlitePool;
use std::{sync::Arc, time::Duration};
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
        let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
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

async fn check_and_run_scheduled_checkins(db: &SqlitePool) -> anyhow::Result<()> {
    let settings = db::get_settings(db).await?;

    if !settings.enabled {
        return Ok(());
    }

    let now = Local::now().time();
    let window_start = NaiveTime::parse_from_str(&settings.window_start, "%H:%M")?;
    let window_end = NaiveTime::parse_from_str(&settings.window_end, "%H:%M")?;

    let in_window = if window_start <= window_end {
        now >= window_start && now <= window_end
    } else {
        now >= window_start || now <= window_end
    };

    if !in_window {
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
        if executed > 0 {
            if let Some(secs) = crate::services::checkin::random_delay_secs(
                settings.batch_delay_min,
                settings.batch_delay_max,
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
            Ok(_) => {
                tracing::info!("Scheduled checkin completed for account {}", account.id);
                // 更新内存计数器，避免后续账户因过期计数而超限
                *today_counts.entry(account.id.clone()).or_insert(0) += 1;
            }
            Err(e) => tracing::error!("Scheduled checkin failed for account {}: {}", account.id, e),
        }
        executed += 1;
    }

    Ok(())
}
