//! Calendar days and wall-clock times. lolcommits runs on the person's own
//! computer, so a day is a day on that computer's clock. All day and clock
//! logic goes through here instead of calling chrono::Local everywhere.
use chrono::{DateTime, Duration, Local, NaiveDate, Timelike};

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The day `days` days before today.
pub fn days_ago(days: i64) -> NaiveDate {
    today() - Duration::days(days)
}

/// Current hour on the clock, 0..=23.
pub fn hour() -> u32 {
    Local::now().hour()
}

/// Unix time of tomorrow at `hour`:00.
pub fn tomorrow_at(hour: u32) -> Option<i64> {
    (today() + Duration::days(1)).and_hms_opt(hour, 0, 0)?.and_local_timezone(Local).earliest().map(|d| d.timestamp())
}

/// "14:05" for a unix time.
pub fn clock(unix: i64) -> String {
    DateTime::from_timestamp(unix, 0).map(|d| d.with_timezone(&Local).format("%H:%M").to_string()).unwrap_or_default()
}
