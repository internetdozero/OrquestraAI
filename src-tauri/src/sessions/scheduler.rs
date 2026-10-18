//! Background auto-purge: the daemon that enforces the retention policy
//! without anyone opening a window.
//!
//! It is deliberately conservative. Nothing runs unless the operator enabled
//! it, pins are always honoured, a session with an unreadable timestamp is
//! never age-purged, and every failure is logged on stderr and swallowed —
//! a scheduler that can crash the app is worse than no scheduler.

use std::time::Duration;

use super::adapters::time::{now_secs, timestamp_secs};
use super::policy;
use super::purge::{self, Store};
use super::types::RetentionPolicy;

/// How often the daemon wakes to re-read the policy.
///
/// This is only the polling granularity: `auto_purge_interval_hours` decides
/// whether a wake-up actually purges, so a policy change is picked up within
/// one tick instead of restarting the interval.
const TICK: Duration = Duration::from_secs(300);

/// Bounds for the stored interval, mirroring the policy store's clamping.
const MIN_INTERVAL_HOURS: u32 = 1;
const MAX_INTERVAL_HOURS: u32 = 720;

/// Start the daemon. Called once from app setup; a thread that cannot be
/// created is reported and the app carries on without a scheduler.
pub fn spawn() {
    let builder = std::thread::Builder::new().name("orquestra-auto-purge".to_string());

    if let Err(error) = builder.spawn(run) {
        crate::log_error!("auto-purge daemon did not start: {error}");
    }
}

fn run() {
    loop {
        std::thread::sleep(TICK);

        if let Err(error) = tick() {
            crate::log_warn!("auto-purge skipped: {error}");
        }
    }
}

/// One wake-up: purge if the policy allows it and the interval has elapsed.
fn tick() -> Result<(), String> {
    let store = Store::system()?;
    let policy = policy::get_policy()?;

    if !due(&policy, now_secs()) {
        return Ok(());
    }

    let ttl_days = |harness: &str| policy.ttl_days_for(harness);
    let selection = purge::select_by_ttl(&store, &ttl_days)?;
    if selection.candidates.is_empty() {
        // Still worth recording: without it the interval would be re-checked
        // on every tick forever.
        policy::mark_last_run(policy::now_iso8601())?;
        return Ok(());
    }

    let report = purge::execute_selection(&store, selection)?;
    crate::log_info!(
        "auto-purge removed {} sessions ({} bytes) with {} failures",
        report.deleted_count, report.reclaimed_bytes, report.failed_count
    );
    for error in &report.errors {
        crate::log_error!("auto-purge: {error}");
    }

    Ok(())
}

/// Is the policy enabled, and has a full interval passed since the last run?
///
/// The decisions are split from [`tick`] so they can be tested without a
/// scheduler thread, a home directory or a database.
pub(crate) fn due(policy: &RetentionPolicy, now: i64) -> bool {
    if !policy.auto_purge_enabled {
        return false;
    }

    let interval = (policy
        .auto_purge_interval_hours
        .clamp(MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS) as i64)
        * 3_600;

    match policy.last_run.as_deref().and_then(timestamp_secs) {
        // A policy that has never run is due immediately.
        None => true,
        Some(last) => now.saturating_sub(last) >= interval,
    }
}
