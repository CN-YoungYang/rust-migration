use crate::{
    db,
    error::{AppError, Result},
    models::AppUser,
    AppState,
};
use axum::{
    extract::{Extension, State},
    Json,
};
use croner::Cron;
use std::str::FromStr;
use std::sync::Arc;

fn require_admin(user: &AppUser) -> Result<()> {
    if user.role != "ADMIN" && user.role != "SUPER_ADMIN" {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
) -> Result<Json<serde_json::Value>> {
    require_admin(&user)?;
    let settings = db::get_settings(&state.db).await?;
    Ok(crate::routes::data(settings))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AppUser>,
    Json(payload): Json<db::UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>> {
    require_admin(&user)?;
    // cron 调度计划校验：非空、数量封顶、每条必须是合法的标准 5 段 cron 表达式。
    if let Some(list) = &payload.schedule_cron {
        if list.is_empty() {
            return Err(AppError::Validation(
                "调度计划至少需要一个 cron 表达式".into(),
            ));
        }
        if list.len() > 20 {
            return Err(AppError::Validation(format!(
                "调度计划最多支持 20 个 cron 表达式，收到 {} 个",
                list.len()
            )));
        }
        for expr in list {
            // croner 默认支持 5~7 段（秒可选），但调度器按分钟粒度截断到秒=0 匹配，
            // 6/7 段表达式里显式的秒字段会永不命中。因此强制标准 5 段。
            let fields: Vec<&str> = expr.split_whitespace().collect();
            if fields.len() != 5 {
                return Err(AppError::Validation(format!(
                    "cron 表达式应为标准 5 段（分 时 日 月 周），当前 {} 段: {expr}",
                    fields.len()
                )));
            }
            if Cron::from_str(expr).is_err() {
                return Err(AppError::Validation(format!(
                    "cron 表达式无效: {expr}（标准 5 段：分 时 日 月 周）"
                )));
            }
        }
    }
    if let Some(max) = payload.max_attempts_per_day {
        if !(1..=100).contains(&max) {
            return Err(AppError::Validation(
                "每天最大尝试次数必须在 1~100 之间".into(),
            ));
        }
    }

    // 批量/定时签到随机延迟范围校验（秒）。允许 min=0 且 max=0 表示不延迟。
    // 约束：0 <= min <= max <= 600（10 分钟封顶，避免单次签到耗时过长）。
    if let (Some(min), Some(max)) = (payload.batch_delay_min, payload.batch_delay_max) {
        if min < 0 || max < 0 || min > max || max > 600 {
            return Err(AppError::Validation(
                "batchDelayMin/Max 必须满足 0 <= min <= max <= 600（秒）".into(),
            ));
        }
    } else if payload.batch_delay_min.is_some() || payload.batch_delay_max.is_some() {
        return Err(AppError::Validation(
            "batchDelayMin 和 batchDelayMax 必须同时提供".into(),
        ));
    }
    // 定时调度签到随机延迟范围校验（秒），规则同上但独立于批量手动签到。
    if let (Some(min), Some(max)) = (payload.scheduled_delay_min, payload.scheduled_delay_max) {
        if min < 0 || max < 0 || min > max || max > 600 {
            return Err(AppError::Validation(
                "scheduledDelayMin/Max 必须满足 0 <= min <= max <= 600（秒）".into(),
            ));
        }
    } else if payload.scheduled_delay_min.is_some() || payload.scheduled_delay_max.is_some() {
        return Err(AppError::Validation(
            "scheduledDelayMin 和 scheduledDelayMax 必须同时提供".into(),
        ));
    }
    if let Some(keep) = payload.cleanup_keep_latest {
        if !(0..=10000).contains(&keep) {
            return Err(AppError::Validation(
                "cleanupKeepLatest 必须在 0~10000 之间（0 表示清除全部）".into(),
            ));
        }
    }

    let settings = db::update_settings(&state.db, &payload).await?;

    Ok(crate::routes::data(settings))
}
