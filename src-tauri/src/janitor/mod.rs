//! Linux system health snapshots and narrowly scoped stale-cache cleanup.

mod health;
mod sweep;
#[cfg(test)]
mod tests;

pub use health::{CpuStats, MemoryStats, SystemHealth, TmpStats, TopProcess};
pub use sweep::SweepReport;

/// Collect live Linux memory, tmpfs, load and process metrics.
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
    let active_sockets = std::fs::read_to_string("/proc/net/unix")
        .map_err(|error| format!("could not inspect live Unix sockets: {error}"))?;

    sweep::sweep_at(
        &home.join(".orquestra"),
        std::path::Path::new("/var/tmp/testing"),
        &active_sockets,
        std::time::SystemTime::now(),
    )
}
