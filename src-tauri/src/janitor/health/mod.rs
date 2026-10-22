//! System health snapshots: the frozen wire shape, one native collector per
//! supported OS, and the platform-neutral math both collectors share.
//!
//! Every field below is serialized camelCase and guarded by the IPC contract
//! test, so a new platform fills this same struct instead of inventing its own
//! payload. Collection lives in `linux` (which reads `/proc`) and `macos`
//! (which asks sysctl and `host_statistics64`); anything reusable and pure
//! stays here so it is testable on any CI host.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "macos", test))]
mod macos;
#[cfg(test)]
mod tests;

use serde::Serialize;
#[cfg(any(target_os = "linux", test))]
use std::collections::HashMap;
#[cfg(unix)]
use std::path::Path;

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

/// Collect live memory, swap, scratch-volume, load and process metrics.
///
/// Linux reads `/proc`, macOS asks sysctl and `host_statistics64`. Other
/// platforms report an explicit error rather than fabricated numbers.
#[cfg(target_os = "linux")]
pub(super) fn snapshot() -> Result<SystemHealth, String> {
    linux::snapshot()
}

#[cfg(target_os = "macos")]
pub(super) fn snapshot() -> Result<SystemHealth, String> {
    macos::snapshot()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(super) fn snapshot() -> Result<SystemHealth, String> {
    Err("system health snapshots are supported on Linux and macOS".to_string())
}

pub(super) fn should_alert(ram_percent: f64, tmp_percent: f64) -> bool {
    ram_percent > 85.0 || tmp_percent > 80.0
}

/// Used memory is `total - free`, so callers pass what the kernel reports as
/// reclaimable-or-idle rather than what is merely unused.
pub(super) fn memory_stats(total_kb: u64, free_kb: u64) -> MemoryStats {
    let used_kb = total_kb.saturating_sub(free_kb);
    let used_percent = if total_kb == 0 {
        0.0
    } else {
        used_kb as f64 * 100.0 / total_kb as f64
    };
    MemoryStats {
        total_mb: total_kb / 1024,
        used_mb: used_kb / 1024,
        free_mb: free_kb.min(total_kb) / 1024,
        used_percent,
    }
}

// ---------------------------------------------------------------------------
// /proc parsers
//
// They are string math, not syscalls, so they compile and run on every host:
// the Linux collector uses them in production and the test suite pins their
// behaviour from any CI runner. Off Linux and outside `cfg(test)` they would
// be dead weight, hence the gate.
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "linux", test))]
pub(super) fn parse_meminfo(source: &str) -> HashMap<String, u64> {
    source
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let kilobytes = value.split_whitespace().next()?.parse().ok()?;
            Some((key.to_string(), kilobytes))
        })
        .collect()
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn required_kb(memory: &HashMap<String, u64>, key: &str) -> Result<u64, String> {
    memory
        .get(key)
        .copied()
        .ok_or_else(|| format!("/proc/meminfo has no {key}"))
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn mount_type_for(source: &str, path: &str) -> Option<String> {
    source
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            fields.next()?;
            let mount = decode_mount_field(fields.next()?);
            let kind = fields.next()?;
            let contains = path == mount
                || (mount == "/" && path.starts_with('/'))
                || path
                    .strip_prefix(&mount)
                    .is_some_and(|suffix| suffix.starts_with('/'));
            contains.then_some((mount.len(), kind.to_string()))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, kind)| kind)
}

#[cfg(any(target_os = "linux", test))]
fn decode_mount_field(field: &str) -> String {
    field
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn parse_load_one(source: &str) -> Result<f64, String> {
    source
        .split_whitespace()
        .next()
        .ok_or_else(|| "/proc/loadavg is empty".to_string())?
        .parse::<f64>()
        .map_err(|error| format!("invalid one-minute load average: {error}"))
}

/// Capacity of the volume behind `path`, in the same shape Linux reports for
/// `/tmp`. `is_tmpfs` is decided by the caller: macOS has no tmpfs, Linux
/// resolves it from `/proc/mounts`.
#[cfg(unix)]
pub(super) fn tmp_stats(path: &Path, is_tmpfs: bool) -> Result<TmpStats, String> {
    use std::os::unix::ffi::OsStrExt;
    let path_c = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| "tmp path contains a NUL byte".to_string())?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path_c` is NUL-terminated and `stats` points to writable storage.
    let result = unsafe { libc::statvfs(path_c.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return Err(format!(
            "could not inspect {} capacity: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: statvfs initialized the structure when it returned success.
    let stats = unsafe { stats.assume_init() };
    #[allow(clippy::unnecessary_cast)]
    let block_size = if stats.f_frsize == 0 {
        stats.f_bsize as u64
    } else {
        stats.f_frsize as u64
    };
    #[allow(clippy::unnecessary_cast)]
    let total_bytes = block_size.saturating_mul(stats.f_blocks as u64);
    #[allow(clippy::unnecessary_cast)]
    let free_bytes = block_size
        .saturating_mul(stats.f_bavail as u64)
        .min(total_bytes);
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    let total_mb = total_bytes / 1_048_576;
    let free_mb = free_bytes / 1_048_576;
    let used_mb = used_bytes / 1_048_576;
    Ok(TmpStats {
        path: path.display().to_string(),
        is_tmpfs,
        total_mb,
        used_mb,
        free_mb,
        used_percent: if total_bytes == 0 {
            0.0
        } else {
            used_bytes as f64 * 100.0 / total_bytes as f64
        },
    })
}
