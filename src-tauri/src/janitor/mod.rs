//! System health snapshots (Linux-only `/proc` metrics) and narrowly scoped
//! stale-cache cleanup. Off Linux the health snapshot reports an explicit
//! error and socket sweeping fails closed: without `/proc/net/unix` liveness
//! cannot be proven, so no socket is ever deleted.

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
