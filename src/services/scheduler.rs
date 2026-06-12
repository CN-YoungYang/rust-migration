use sqlx::SqlitePool;
use tokio_cron_scheduler::{JobScheduler, Job};
use chrono::{Local, NaiveTime, Utc};
use std::sync::Arc;
use tokio::sync::Semaphore;
use crate::{
    db,
    services::checkin::runner::execute_checkin,
};

const MAX_CONCURRENT_CHECKINS: usize = 10;

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

    let accounts = db::list_accounts(db).await?;
    let today_local = Local::now().date_naive();

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CHECKINS));
    let mut tasks = Vec::new();

    for account in accounts {
        if !account.enabled {
            continue;
        }

        // Already succeeded today (in local time) -> skip
        if let Some(last_run) = account.last_run_at {
            let last_run_local = last_run.with_timezone(&Local);
            if last_run_local.date_naive() == today_local {
                if let Some(status) = &account.last_status {
                    if status == "success" || status == "already_checked" {
                        continue;
                    }
                }

                // Check if retry is allowed (both global and per-account)
                if !settings.retry_enabled || !account.retry_enabled {
                    continue;
                }
            }
        }

        // Enforce maxAttemptsPerDay: count today's runs for this account
        let today_runs = db::count_runs_by_account_today(db, &account.id).await?;
        if today_runs >= settings.max_attempts_per_day.max(1) {
            tracing::debug!(
                "Skipping account {}: {}/{} attempts today",
                account.id, today_runs, settings.max_attempts_per_day
            );
            continue;
        }

        let db = db.clone();
        let account_id = account.id.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            match execute_checkin(&db, &account_id, "scheduled").await {
                Ok(_) => tracing::info!("Scheduled checkin completed for account {}", account_id),
                Err(e) => tracing::error!("Scheduled checkin failed for account {}: {}", account_id, e),
            }
        }));
    }

    for task in tasks {
        let _ = task.await;
    }

    Ok(())
}
