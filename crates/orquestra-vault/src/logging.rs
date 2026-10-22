//! Minimal structured logging, mirroring the shell's `crate::log` format.
//!
//! The moved code warns from failure paths (keyring unreachable, failover
//! skipped, migration backups swept); each line keeps the
//! `<timestamp> [<module_path>] LEVEL  <message>` shape the app's logs use,
//! so the crate can log without depending on the shell.

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

/// Log a warning to stderr with timestamp and call-site module.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {{
        eprintln!(
            "{} [{}] WARN  {}",
            $crate::logging::now(),
            module_path!(),
            format!($($arg)*)
        )
    }};
}
