//! macOS health collection, split in two halves on purpose.
//!
//! The pure half (page-counter arithmetic and the `/bin/ps` parser) has no
//! syscalls, so it compiles and is tested on any host. The native half lives
//! in `macos::native`, which only exists on Darwin: sysctl, `host_statistics64`
//! and `/bin/ps` never leak into a Linux build.

use super::{MemoryStats, TopProcess};

/// Page counters from Darwin's `host_statistics64`, reduced to the handful
/// this app reports. Kept as plain integers so the arithmetic is testable
/// without a Mac.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct DarwinMemoryCounts {
    pub free: u64,
    pub speculative: u64,
    pub active: u64,
    pub wired: u64,
    pub compressor: u64,
}

/// RAM the way Activity Monitor tells it apart: *free* is idle plus
/// speculative pages, *used* is active, wired and compressed. Unlike the Linux
/// collector this is not `total - free`, because Darwin keeps file cache in a
/// separate, reclaimable bucket that Activity Monitor does not count as used.
pub(super) fn darwin_memory_stats(
    total_bytes: u64,
    page_size: u64,
    counts: DarwinMemoryCounts,
) -> MemoryStats {
    let total_kb = total_bytes / 1024;
    let per_page_kb = page_size / 1024;
    let free_kb = counts
        .free
        .saturating_add(counts.speculative)
        .saturating_mul(per_page_kb)
        .min(total_kb);
    let used_kb = counts
        .active
        .saturating_add(counts.wired)
        .saturating_add(counts.compressor)
        .saturating_mul(per_page_kb)
        .min(total_kb);
    MemoryStats {
        total_mb: total_kb / 1024,
        used_mb: used_kb / 1024,
        free_mb: free_kb / 1024,
        used_percent: if total_kb == 0 {
            0.0
        } else {
            used_kb as f64 * 100.0 / total_kb as f64
        },
    }
}

/// Rows of `ps -axo pid=,rss=,comm=`: pid, resident set in KiB, then the
/// executable path, which may contain spaces. Unparseable rows are dropped
/// instead of aborting the list.
pub(super) fn parse_ps(source: &str) -> Vec<TopProcess> {
    let mut processes = source
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse::<u32>().ok()?;
            let rss_kb = fields.next()?.parse::<u64>().ok()?;
            let name = fields.collect::<Vec<_>>().join(" ");
            if name.is_empty() {
                return None;
            }
            Some(TopProcess {
                pid,
                name,
                rss_mb: rss_kb / 1024,
            })
        })
        .collect::<Vec<_>>();
    processes.sort_by(|left, right| {
        right
            .rss_mb
            .cmp(&left.rss_mb)
            .then_with(|| left.pid.cmp(&right.pid))
    });
    processes
}

#[cfg(target_os = "macos")]
mod native;

#[cfg(target_os = "macos")]
pub(super) fn snapshot() -> Result<super::SystemHealth, String> {
    native::snapshot()
}
