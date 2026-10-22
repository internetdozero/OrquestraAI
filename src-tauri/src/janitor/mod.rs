//! System health snapshots and narrowly scoped stale-cache cleanup.
//!
//! Health is collected natively per OS: `/proc` on Linux, sysctl and
//! `host_statistics64` on macOS. On any other platform the snapshot reports
//! an explicit error instead of inventing numbers. Socket sweeping reads
//! liveness from `/proc/net/unix`, which only Linux has; off Linux it fails
//! closed, so no socket is ever deleted on a guess.

mod health;
mod sweep;
#[cfg(test)]
mod tests;

pub use health::{CpuStats, MemoryStats, SystemHealth, TmpStats, TopProcess};
pub use sweep::SweepReport;

/// Collect live memory, swap, scratch-volume, load and process metrics from
/// the platform's native source. Returns an explicit error on platforms
/// without a collector.
#[tauri::command]
pub fn get_system_health() -> Result<SystemHealth, String> {
    health::snapshot()
}

/// Remove only dead Orquestra socket files and aged direct-child scratch files.
#[tauri::command]
pub fn sweep_system_caches() -> Result<SweepReport, String> {
    let home = crate::paths::require_home("locate ~/.orquestra")?;
    // `/proc/net/unix` is Linux-only. Elsewhere liveness is reported as
    // `None`, which `sweep::clean_sockets` reads as "unknown, delete
    // nothing" — an empty table would instead mean "no socket is live" and
    // would unlink the running instance's own socket on macOS.
    #[cfg(target_os = "linux")]
    let active_sockets: Option<String> = Some(
        std::fs::read_to_string("/proc/net/unix")
            .map_err(|error| format!("could not inspect live Unix sockets: {error}"))?,
    );
    #[cfg(not(target_os = "linux"))]
    let active_sockets: Option<String> = None;

    sweep::sweep_at(
        &home.join(".orquestra"),
        &crate::paths::scratch_root(),
        active_sockets.as_deref(),
        std::time::SystemTime::now(),
    )
}
