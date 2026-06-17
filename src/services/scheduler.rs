use sqlx::SqlitePool;
use tokio_cron_scheduler::{JobScheduler, Job};
use chrono::{Local, NaiveTime};
use crate::{
    db,
    services::checkin::runner::{execute_checkin, skip_reason_for_batch},
};

pub async fn start_scheduler(db: SqlitePool) {
    tokio::spawn(async move {
        if let Err(e) = run_scheduler(db).await {
            tracing::error!("Scheduler error: {}", e);
        }
    });
}

async fn run_scheduler(db: SqlitePool) -> anyhow::Result<()> {
    let scheduler = JobScheduler::new().await?;

    // Checkin job every 5 minutes
    let db_clone = db.clone();
    scheduler.add(
        Job::new_async("0 */5 * * * *", move |_uuid, _l| {
            let db = db_clone.clone();
            Box::pin(async move {
                if let Err(e) = check_and_run_scheduled_checkins(&db).await {
                    tracing::error!("Scheduled checkin error: {}", e);
                }
            })
        })?
    ).await?;

    // Run cleanup every 10 minutes
    let db_clone = db.clone();
    scheduler.add(
        Job::new_async("0 */10 * * * *", move |_uuid, _l| {
            let db = db_clone.clone();
            Box::pin(async move {
                cleanup_old_runs(&db).await;
            })
        })?
    ).await?;

    scheduler.start().await?;
    tracing::info!("Scheduler started");

    // Keep task alive
    std::future::pending::<()>().await;
    Ok(())
}

async fn cleanup_old_runs(db: &SqlitePool) {
    if let Err(e) = db::cleanup_checkin_runs(db, 500).await {
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

    let mut accounts = db::list_accounts(db).await?;
    let today_local = Local::now().date_naive();

    // 批量查询今日各账户签到次数，避免逐账户 COUNT
    let today_counts = db::count_runs_today_batch(db).await.unwrap_or_default();

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

        // Enforce maxAttemptsPerDay: 使用批量查询结果
        let today_runs = today_counts.get(&account.id).copied().unwrap_or(0);
        if today_runs >= settings.max_attempts_per_day.max(1) {
            tracing::debug!(
                "Skipping account {}: {}/{} attempts today",
                account.id, today_runs, settings.max_attempts_per_day
            );
            continue;
        }

        // 首个账户不延迟，其余账户签到前随机 sleep（按管理员设置）
        if executed > 0 {
            if let Some(secs) = crate::services::checkin::random_delay_secs(
                settings.batch_delay_min,
                settings.batch_delay_max,
            ) {
                tracing::debug!("Scheduled checkin: account {} waiting {}s", account.id, secs);
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            }
        }

        match execute_checkin(db, &account.id, "scheduled").await {
            Ok(_) => tracing::info!("Scheduled checkin completed for account {}", account.id),
            Err(e) => tracing::error!("Scheduled checkin failed for account {}: {}", account.id, e),
        }
        executed += 1;
    }

    Ok(())
}
