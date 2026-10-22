//! System health snapshots (Linux-only `/proc` metrics) and narrowly scoped
//! stale-cache cleanup. Off Linux the health snapshot reports an explicit
//! error and socket sweeping is a no-op: without `/proc/net/unix` liveness
//! cannot be proven, so nothing is deleted.

mod health;
mod sweep;
#[cfg(test)]
mod tests;

pub use health::{CpuStats, MemoryStats, SystemHealth, TmpStats, TopProcess};
pub use sweep::SweepReport;

/// Collect live Linux memory, tmpfs, load and process metrics.
/// Returns an explicit error off Linux, where `/proc` does not exist.
#[tauri::command]
pub fn get_system_health() -> Result<SystemHealth, String> {
    health::snapshot()
}

/// Remove only dead Orquestra socket files and aged direct-child scratch files.
#[tauri::command]
pub fn sweep_system_caches() -> Result<SweepReport, String> {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "HOME is not set; cannot locate ~/.orquestra".to_string())?;
    // `/proc/net/unix` is Linux-only; elsewhere socket sweeping is a
    // no-op (see `sweep::clean_sockets`), so there is nothing to match.
    #[cfg(target_os = "linux")]
    let active_sockets = std::fs::read_to_string("/proc/net/unix")
        .map_err(|error| format!("could not inspect live Unix sockets: {error}"))?;
    #[cfg(not(target_os = "linux"))]
    let active_sockets = String::new();

    sweep::sweep_at(
        &home.join(".orquestra"),
        &crate::paths::scratch_root(),
        &active_sockets,
        std::time::SystemTime::now(),
    )
}
