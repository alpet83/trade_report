use chrono::{DateTime, Utc, Duration, Datelike, Timelike, Weekday};

// Constants for time intervals in seconds
pub const HOUR_SECONDS: i64 = 3600;
pub const DAY_SECONDS: i64 = 24 * HOUR_SECONDS;
pub const WEEK_SECONDS: i64 = 7 * DAY_SECONDS;
pub const MONTH_SECONDS: i64 = 30 * DAY_SECONDS;
pub const QUARTER_SECONDS: i64 = 90 * DAY_SECONDS;
pub const YEAR_SECONDS: i64 = 365 * DAY_SECONDS;

// Creates a DateTime<Utc> with specified components, setting time to 00:00:00.000
pub fn make_datetime(
    ts: DateTime<Utc>,
    day: Option<u32>,
    month: Option<u32>,
    year: Option<i32>,
) -> DateTime<Utc> {
    let mut new_ts = ts;
    if let Some(d) = day {
        new_ts = new_ts.with_day(d).expect("Invalid day");
    }
    if let Some(m) = month {
        new_ts = new_ts.with_month(m).expect("Invalid month");
    }
    if let Some(y) = year {
        new_ts = new_ts.with_year(y).expect("Invalid year");
    }
    new_ts
        .with_hour(0)
        .expect("Invalid hour")
        .with_minute(0)
        .expect("Invalid minute")
        .with_second(0)
        .expect("Invalid second")
        .with_nanosecond(0)
        .expect("Invalid nanosecond")
}

// Adjusts timestamp to the Monday of the week containing the given date at 00:00:00.000
pub fn adjust_to_monday(ts: DateTime<Utc>) -> DateTime<Utc> {
    let mut adjusted = make_datetime(ts, None, None, None); // Reset time to 00:00:00.000
    while adjusted.weekday() != Weekday::Mon || adjusted.hour() != 0 {
        adjusted = adjusted - Duration::hours(1);
    }
    adjusted
}

// Adjusts timestamp to the first day of the month, optionally aligning to Monday of the week containing the 1st
pub fn adjust_to_first_of_month(ts: DateTime<Utc>, interval_seconds: i64, week_align: bool) -> DateTime<Utc> {
    let is_monthly_or_larger = interval_seconds >= MONTH_SECONDS;
    let mut adjusted = if is_monthly_or_larger && ts.month() == 12 && ts.day() >= 25 {
        // Shift end-of-year trades to the next month
        let next_month = ts + Duration::days(7);
        make_datetime(next_month, Some(1), None, None)
    } else {
        make_datetime(ts, Some(1), None, None)
    };
    if week_align && interval_seconds >= WEEK_SECONDS {
        adjusted = adjust_to_monday(adjusted);
    }
    adjusted
}

// Adjusts timestamp to the first day of the quarter, optionally aligning to Monday of the week containing the 1st
pub fn adjust_to_first_of_quarter(ts: DateTime<Utc>, week_align: bool) -> DateTime<Utc> {
    let month = ts.month();
    let quarter_start_month = match month {
        1..=3 => 1,
        4..=6 => 4,
        7..=9 => 7,
        _ => 10,
    };
    let adjusted = make_datetime(ts, Some(1), Some(quarter_start_month), None);
    if week_align {
        adjust_to_monday(adjusted)
    } else {
        adjusted
    }
}

// Adjusts timestamp to the first day of the year, optionally aligning to Monday of the week containing Jan 1
pub fn adjust_to_first_of_year(ts: DateTime<Utc>, week_align: bool) -> DateTime<Utc> {
    let adjusted = make_datetime(ts, Some(1), Some(1), None);
    if week_align {
        adjust_to_monday(adjusted)
    } else {
        adjusted
    }
}