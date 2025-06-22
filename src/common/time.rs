// /src/common/time.rs
// Modified: 2025-06-22 10:40:00 EEST

use chrono::{DateTime, Utc, Timelike};
use crate::logs::app_error::AppError;

// Resolves time range based on start_ts, end_ts, or period
pub async fn resolve_time_range(
    start_ts: Option<String>,
    end_ts: Option<String>,
    period: Option<i64>,
    period_type: Option<String>,
) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
    let end_ts = match end_ts {
        Some(end) => DateTime::parse_from_rfc3339(&end)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AppError::Internal(format!("Invalid end_ts format: {}", e)))?,
        None => Utc::now(),
    };
    let start_ts = match start_ts {
        Some(start) => DateTime::parse_from_rfc3339(&start)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AppError::Internal(format!("Invalid start_ts format: {}", e)))?,
        None => {
            let hours = match period_type.as_deref() {
                Some("weekly") => 365 * 24, // 1 year
                _ => period.unwrap_or(24),
            };
            (end_ts - chrono::Duration::hours(hours))
                .with_minute(0)
                .expect("Invalid datetime")
                .with_second(0)
                .expect("Invalid datetime")
                .with_nanosecond(0)
                .expect("Invalid datetime")
        }
    };
    Ok((start_ts, end_ts))
}