use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use crate::sys_time_millis;

pub fn sys_time() -> NaiveDateTime {
    DateTime::<Utc>::from_timestamp_millis(sys_time_millis())
        .expect("invalid timestamp")
        .naive_utc()
}

pub fn sleep(millisecs: i64) {
    let start = sys_time();
    let dur = Duration::milliseconds(millisecs);
    let mut current = start;

    while current - start < dur {
        current = sys_time();
        core::hint::spin_loop();
    }
}
