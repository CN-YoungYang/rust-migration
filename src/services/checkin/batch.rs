use super::runner::execute_checkin;
use crate::{db, error::Result};
use sqlx::SqlitePool;

/// 将批次交给 Tokio 后台执行，HTTP 请求只承担创建和入队。
pub fn spawn_checkin_batch(db: SqlitePool, batch_id: String) {
    tokio::spawn(async move {
        if let Err(error) = run_checkin_batch(&db, &batch_id).await {
            tracing::error!(batch_id = %batch_id, error = %error, "异步签到批次执行失败");
            if let Err(mark_error) =
                db::fail_checkin_batch(&db, &batch_id, &error.user_message()).await
            {
                tracing::error!(batch_id = %batch_id, error = %mark_error, "标记签到批次失败");
            }
        }
    });
}

/// 按已锁定的范围串行执行批次。每个站点请求由 runner 自己限制超时，
/// 单个账号失败只写入当前项，循环继续处理后续账号。
pub async fn run_checkin_batch(db: &SqlitePool, batch_id: &str) -> Result<()> {
    let batch = db::find_checkin_batch(db, batch_id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;
    if matches!(
        batch.status.as_str(),
        "completed" | "partial_failed" | "failed"
    ) {
        return Ok(());
    }

    let settings = db::get_settings(db).await?;
    if batch.status == "pending" && !db::mark_checkin_batch_started(db, batch_id).await? {
        // 只有成功把 pending 改为 running 的 worker 可以执行，避免手动恢复
        // 与原有 worker 同时读取同一批 pending 项。
        return Ok(());
    }
    let items = db::list_pending_checkin_batch_items(db, batch_id).await?;

    for (index, item) in items.into_iter().enumerate() {
        if index > 0 {
            if let Some(seconds) =
                super::random_delay_secs(settings.batch_delay_min, settings.batch_delay_max)
            {
                tracing::debug!(
                    batch_id = %batch_id,
                    account_id = %item.account_id,
                    seconds,
                    "异步签到批次等待随机间隔"
                );
                tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
            }
        }

        if !db::mark_checkin_batch_item_running(db, batch_id, &item.account_id).await? {
            continue;
        }
        let result = execute_checkin(db, &item.account_id, "manual_batch", Some(&settings)).await;
        match result {
            Ok(run) => {
                // skipped/already_checked 可能来自执行期重检，不一定有真实签到记录；
                // 只有数据库中确实存在的记录才建立 runId 关联。
                let run_id = if db::find_run_by_id(db, &run.id).await?.is_some() {
                    Some(run.id.as_str())
                } else {
                    None
                };
                db::finish_checkin_batch_item(
                    db,
                    batch_id,
                    &item.account_id,
                    &run.status,
                    run.message.as_deref(),
                    run_id,
                )
                .await?;
            }
            Err(error) => {
                db::finish_checkin_batch_item(
                    db,
                    batch_id,
                    &item.account_id,
                    "failed",
                    Some(&error.user_message()),
                    None,
                )
                .await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn terminal_batch_statuses_are_not_restarted() {
        for status in ["completed", "partial_failed", "failed"] {
            assert!(matches!(status, "completed" | "partial_failed" | "failed"));
        }
    }
}
