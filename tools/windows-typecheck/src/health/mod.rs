//! Parent-side context for `src-tauri/src/janitor/health/windows.rs`.
//!
//! The real module imports the wire shape and two helpers from the health
//! parent module, and asks `crate::paths` for the scratch root. The collector's
//! own structs are plain `Serialize` types with no behaviour, so restating
//! them here is safe; the arithmetic helper and the alert threshold are
//! restated too, because they are `pub(super)` and therefore invisible from
//! here.
//!
//! Drift between these stand-ins and the real ones cannot silence a compile
//! error: they are copied verbatim from `src-tauri/src/janitor/health/mod.rs`
//! and any signature change shows up as a mismatch when this crate is built.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStats {
    pub total_mb: u64,
    pub used_mb: u64,
    pub free_mb: u64,
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TmpStats {
    pub path: String,
    pub is_tmpfs: bool,
    pub total_mb: u64,
    pub used_mb: u64,
    pub free_mb: u64,
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuStats {
    pub cores: usize,
    pub load_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopProcess {
    pub pid: u32,
    pub name: String,
    pub rss_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    pub ram: MemoryStats,
    pub swap: MemoryStats,
    pub tmp: TmpStats,
    pub cpu: CpuStats,
    pub top_processes: Vec<TopProcess>,
    pub alert: bool,
}

pub(super) fn should_alert(ram_percent: f64, tmp_percent: f64) -> bool {
    ram_percent > 85.0 || tmp_percent > 80.0
}

pub(super) fn memory_stats(total_kb: u64, free_kb: u64) -> MemoryStats {
    let used_kb = total_kb.saturating_sub(free_kb);
    MemoryStats {
        total_mb: total_kb / 1024,
        used_mb: used_kb / 1024,
        free_mb: free_kb.min(total_kb) / 1024,
        used_percent: if total_kb == 0 {
            0.0
        } else {
            used_kb as f64 * 100.0 / total_kb as f64
        },
    }
}

/// The real collector, compiled from the desktop shell. `super::` inside it
/// resolves to this module; its own `mod native;` follows the file's real
/// directory (`src-tauri/src/janitor/health/windows/`).
#[path = "../../../../src-tauri/src/janitor/health/windows.rs"]
mod windows;

/// What the desktop shell calls on Windows (`janitor::health::snapshot`).
pub fn snapshot() -> Result<SystemHealth, String> {
    windows::snapshot()
}
