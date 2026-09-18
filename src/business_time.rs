use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};

/// 平台业务时区。签到“今日”、每日尝试上限和运营统计都使用同一时区。
pub const TIMEZONE_NAME: &str = "Asia/Shanghai";
const OFFSET_SECONDS: i32 = 8 * 60 * 60;

pub fn timezone() -> FixedOffset {
    FixedOffset::east_opt(OFFSET_SECONDS).expect("Asia/Shanghai offset is valid")
}

pub fn now() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&timezone())
}

pub fn today() -> NaiveDate {
    now().date_naive()
}

pub fn date_in_business_timezone(value: DateTime<Utc>) -> NaiveDate {
    value.with_timezone(&timezone()).date_naive()
}

pub fn day_start_utc(date: NaiveDate) -> crate::error::Result<DateTime<Utc>> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| crate::error::AppError::Internal("无法计算业务日期开始时间".into()))?;
    local_datetime_to_utc(naive, "业务日期开始时间")
}

pub fn day_end_utc(date: NaiveDate) -> crate::error::Result<DateTime<Utc>> {
    let naive = date
        .and_hms_milli_opt(23, 59, 59, 999)
        .ok_or_else(|| crate::error::AppError::Internal("无法计算业务日期结束时间".into()))?;
    local_datetime_to_utc(naive, "业务日期结束时间")
}

fn local_datetime_to_utc(naive: NaiveDateTime, label: &str) -> crate::error::Result<DateTime<Utc>> {
    timezone()
        .from_local_datetime(&naive)
        .single()
        .map(|value| value.to_utc())
        .ok_or_else(|| crate::error::AppError::Internal(format!("无法解析{label}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn business_day_boundaries_use_east_eight_offset() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 17).unwrap();
        let start = day_start_utc(date).unwrap();
        let end = day_end_utc(date).unwrap();
        assert_eq!(start.hour(), 16);
        assert_eq!(start.day(), 16);
        assert_eq!(end.hour(), 15);
        assert_eq!(end.day(), 17);
        assert_eq!(date_in_business_timezone(start), date);
    }
}
