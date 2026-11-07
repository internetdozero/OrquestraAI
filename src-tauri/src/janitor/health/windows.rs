//! Windows health collection, split the same way as the macOS collector.
//!
//! The pure half — page-file arithmetic, `GetSystemTimes` utilization and the
//! process-name decoder — has no syscalls, so it compiles and is tested on any
//! host. The native half lives in `windows::native`, which only exists on
//! Windows: `GlobalMemoryStatusEx`, `EnumProcesses` and `GetSystemTimes` never
//! leak into a Linux build.

#[cfg(target_os = "windows")]
mod native;
#[cfg(target_os = "windows")]
use super::{should_alert, CpuStats, SystemHealth, TmpStats, TopProcess};
#[cfg(target_os = "windows")]
use std::path::Path;

/// Windows reports a combined "commit limit": the page file covers RAM plus
/// swap, so the swap figures are the difference, clamped at zero. Returns
/// `(total_kb, free_kb)` ready for [`memory_stats`].
pub(super) fn swap_stats(
    total_phys: u64,
    avail_phys: u64,
    total_page_file: u64,
    avail_page_file: u64,
) -> (u64, u64) {
    let total_kb = total_page_file.saturating_sub(total_phys) / 1024;
    let free_kb = avail_page_file.saturating_sub(avail_phys) / 1024;
    (total_kb, free_kb)
}

/// CPU utilization (0–100) from cumulative `GetSystemTimes` counters.
///
/// Windows counts idle time *inside* kernel time, so busy is
/// `kernel + user - idle` and total is `kernel + user`. This is the
/// since-boot average, which is what the counters offer without sampling; the
/// UI labels it as utilization rather than the per-core load average Linux
/// reports.
pub(super) fn cpu_utilization(idle: u64, kernel: u64, user: u64) -> f64 {
    let total = kernel.saturating_add(user);
    if total == 0 {
        return 0.0;
    }
    let busy = total.saturating_sub(idle.min(total));
    busy as f64 * 100.0 / total as f64
}

/// File name of a full Windows image path, without the `.exe` suffix.
///
/// `QueryFullProcessImageNameW` returns `C:\…\chrome.exe`; the process list
/// shows `chrome`. Anything without a separator is returned as-is, so PID-less
/// or path-less answers still get a label.
pub(super) fn process_name_from_path(path: &str) -> String {
    let base = path
        .rsplit(['\\', '/'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path);
    let trimmed = base
        .strip_suffix(".exe")
        .or_else(|| base.strip_suffix(".EXE"))
        .unwrap_or(base);
    trimmed.to_string()
}

/// Capacity of the volume behind `path`, in the same shape Linux reports for
/// `/tmp`. Windows has no tmpfs, so `is_tmpfs` is always false.
#[cfg(target_os = "windows")]
fn volume_capacity(path: &Path) -> Result<TmpStats, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let (mut free_to_caller, mut total, mut free_total) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` is NUL-terminated and outlives the call; the three out
    // pointers are valid, distinct, properly aligned `u64`s.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_to_caller as *mut u64,
            &mut total,
            &mut free_total,
        )
    };
    if ok == 0 {
        return Err(format!(
            "could not inspect {} capacity: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(capacity_from_bytes(path, total, free_to_caller))
}

/// Same arithmetic as the Unix `statvfs` path, from raw byte counts.
#[cfg(target_os = "windows")]
fn capacity_from_bytes(path: &Path, total_bytes: u64, free_bytes: u64) -> TmpStats {
    let free_bytes = free_bytes.min(total_bytes);
    let used_bytes = total_bytes - free_bytes;
    TmpStats {
        path: path.display().to_string(),
        is_tmpfs: false,
        total_mb: total_bytes / 1_048_576,
        used_mb: used_bytes / 1_048_576,
        free_mb: free_bytes / 1_048_576,
        used_percent: if total_bytes == 0 {
            0.0
        } else {
            used_bytes as f64 * 100.0 / total_bytes as f64
        },
    }
}

#[cfg(target_os = "windows")]
pub(super) fn snapshot() -> Result<SystemHealth, String> {
    let (ram, swap) = native::memory()?;
    let tmp = volume_capacity(&crate::paths::scratch_root())?;
    let cores = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .max(1);
    let cpu = CpuStats {
        cores,
        load_percent: native::cpu_utilization()?,
    };
    let top_processes = native::top_processes(5);
    let alert = should_alert(ram.used_percent, tmp.used_percent);
    Ok(SystemHealth {
        ram,
        swap,
        tmp,
        cpu,
        top_processes,
        alert,
    })
}

/// Order the collector's processes: heaviest RSS first, ties broken by pid so
/// two runs over the same data agree. Windows-only, and covered by the macOS
/// suite that ranks the same way.
#[cfg(target_os = "windows")]
pub(super) fn rank_by_rss(mut processes: Vec<TopProcess>, limit: usize) -> Vec<TopProcess> {
    processes.sort_by(|left, right| {
        right
            .rss_mb
            .cmp(&left.rss_mb)
            .then_with(|| left.pid.cmp(&right.pid))
    });
    processes.truncate(limit);
    processes
}
