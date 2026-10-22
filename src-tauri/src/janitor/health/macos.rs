//! macOS health collection, split in two halves on purpose.
//!
//! The pure half (page-counter arithmetic and the decoders for libproc's
//! buffers) has no syscalls, so it compiles and is tested on any host. The
//! native half lives in `macos::native`, which only exists on Darwin: sysctl,
//! `host_statistics64` and libproc never leak into a Linux build.

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

/// Resident bytes from a `proc_taskinfo` buffer.
///
/// The kernel writes the struct for `PROC_PIDTASKINFO`; its first two fields
/// are `u64 pti_virtual_size` and `u64 pti_resident_size`, unchanged since
/// 10.5. Only those 16 leading bytes are read, so a struct that grows a field
/// later cannot shift what this code trusts — and a buffer shorter than the
/// two words yields `None` instead of a truncated number.
pub(super) fn resident_bytes(taskinfo: &[u8]) -> Option<u64> {
    let resident = taskinfo.get(8..16)?;
    let mut raw = [0u8; 8];
    raw.copy_from_slice(resident);
    Some(u64::from_ne_bytes(raw))
}

/// Name from a `proc_name` buffer, which the kernel NUL-terminates. Trailing
/// bytes after the terminator are ignored, so a reused buffer cannot leak an
/// older, longer name.
pub(super) fn process_name(buffer: &[u8]) -> Option<String> {
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    let name = std::str::from_utf8(&buffer[..end]).ok()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Keep the `limit` heaviest processes by resident size, ties broken by pid so
/// the panel does not reshuffle between refreshes. Pure, so the ordering is
/// pinned by a test that runs on any host.
pub(super) fn rank_by_rss(processes: &mut Vec<TopProcess>, limit: usize) {
    processes.sort_by(|left, right| {
        right
            .rss_mb
            .cmp(&left.rss_mb)
            .then_with(|| left.pid.cmp(&right.pid))
    });
    processes.truncate(limit);
}

#[cfg(target_os = "macos")]
mod native;

#[cfg(target_os = "macos")]
pub(super) fn snapshot() -> Result<super::SystemHealth, String> {
    native::snapshot()
}
