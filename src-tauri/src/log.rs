//! Minimal structured logging without extra dependencies.
//!
//! Emits `<timestamp> [<module_path>] LEVEL  <message>` to stderr so every
//! production log line carries a UTC timestamp and its call-site module.

use std::time::{SystemTime, UNIX_EPOCH};

/// UTC wall-clock derived from [`SystemTime`] (no chrono dependency).
///
/// Format is a simplified `epoch+Nd HH:MM:SSZ` rendering of UNIX time.
pub(crate) fn now() -> String {
    let secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_secs(),
        Err(_) => 0,
    };
    let days = secs / 86_400;
    let rem = secs % 86_400;
    format!(
        "epoch+{}d {:02}:{:02}:{:02}Z",
        days,
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

/// Log an error to stderr with timestamp and call-site module.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        eprintln!(
            "{} [{}] ERROR  {}",
            $crate::log::now(),
            module_path!(),
            format!($($arg)*)
        )
    }};
}

/// Log a warning to stderr with timestamp and call-site module.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {{
        eprintln!(
            "{} [{}] WARN  {}",
            $crate::log::now(),
            module_path!(),
            format!($($arg)*)
        )
    }};
}

/// Log an informational line to stderr with timestamp and call-site module.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        eprintln!(
            "{} [{}] INFO  {}",
            $crate::log::now(),
            module_path!(),
            format!($($arg)*)
        )
    }};
}
