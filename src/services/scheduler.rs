use sqlx::SqlitePool;
use tokio_cron_scheduler::{JobScheduler, Job};
use chrono::{Local, NaiveTime};
use crate::{
    db,
    services::checkin::runner::execute_checkin,
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
    
    // Run every 5 minutes
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
    
    scheduler.start().await?;
    tracing::info!("Scheduler started");
    
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
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
    
    for account in accounts {
        if !account.enabled {
            continue;
        }
        
        // Check if already ran today successfully
        if let Some(last_run) = account.last_run_at {
            let today = Local::now().date_naive();
            let last_run_date = last_run.date_naive();
            
            if last_run_date == today {
                if let Some(status) = &account.last_status {
                    if status == "success" || status == "already_checked" {
                        continue;
                    }
                }
            }
        }
        
        tokio::spawn({
            let db = db.clone();
            let account_id = account.id.clone();
            async move {
                match execute_checkin(&db, &account_id, "scheduled").await {
                    Ok(_) => tracing::info!("Scheduled checkin completed for account {}", account_id),
                    Err(e) => tracing::error!("Scheduled checkin failed for account {}: {}", account_id, e),
                }
            }
        });
    }
    
    Ok(())
}
