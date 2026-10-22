//! Linux health collection: memory, swap, `/tmp` capacity, load and the top
//! RSS consumers, all read from `/proc`.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use super::{
    memory_stats, mount_type_for, parse_load_one, parse_meminfo, required_kb, should_alert,
    tmp_stats, CpuStats, SystemHealth, TopProcess,
};

pub(super) fn snapshot() -> Result<SystemHealth, String> {
    let meminfo = fs::read_to_string("/proc/meminfo")
        .map_err(|error| format!("could not read /proc/meminfo: {error}"))?;
    let memory = parse_meminfo(&meminfo);
    let ram_total = required_kb(&memory, "MemTotal")?;
    let ram_free = memory
        .get("MemAvailable")
        .copied()
        .or_else(|| memory.get("MemFree").copied())
        .ok_or_else(|| "/proc/meminfo has no MemAvailable or MemFree".to_string())?;
    let swap_total = memory.get("SwapTotal").copied().unwrap_or(0);
    let swap_free = memory.get("SwapFree").copied().unwrap_or(0);
    let ram = memory_stats(ram_total, ram_free);
    let swap = memory_stats(swap_total, swap_free);

    let mounts = fs::read_to_string("/proc/mounts")
        .map_err(|error| format!("could not read /proc/mounts: {error}"))?;
    let is_tmpfs = mount_type_for(&mounts, "/tmp").as_deref() == Some("tmpfs");
    let tmp = tmp_stats(Path::new("/tmp"), is_tmpfs)?;

    let loadavg = fs::read_to_string("/proc/loadavg")
        .map_err(|error| format!("could not read /proc/loadavg: {error}"))?;
    let load_one = parse_load_one(&loadavg)?;
    let cores = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .max(1);
    let cpu = CpuStats {
        cores,
        load_percent: load_one * 100.0 / cores as f64,
    };

    let top_processes = top_user_processes(5);
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

fn top_user_processes(limit: usize) -> Vec<TopProcess> {
    let Ok(current) = fs::metadata("/proc/self") else {
        return Vec::new();
    };
    let uid = current.uid();
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut processes = entries
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse::<u32>().ok()?;
            let directory = entry.path();
            let metadata = fs::metadata(&directory).ok()?;
            if metadata.uid() != uid {
                return None;
            }
            let status = fs::read_to_string(directory.join("status")).ok()?;
            let rss_kb = status.lines().find_map(|line| {
                line.strip_prefix("VmRSS:")?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()
            })?;
            let name = fs::read_to_string(directory.join("comm"))
                .ok()?
                .trim()
                .to_string();
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
    processes.truncate(limit);
    processes
}
