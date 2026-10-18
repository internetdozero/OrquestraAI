use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
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

pub(super) fn should_alert(ram_percent: f64, tmp_percent: f64) -> bool {
    ram_percent > 85.0 || tmp_percent > 80.0
}

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

pub(super) fn required_kb(memory: &HashMap<String, u64>, key: &str) -> Result<u64, String> {
    memory
        .get(key)
        .copied()
        .ok_or_else(|| format!("/proc/meminfo has no {key}"))
}

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

fn decode_mount_field(field: &str) -> String {
    field
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

pub(super) fn parse_load_one(source: &str) -> Result<f64, String> {
    source
        .split_whitespace()
        .next()
        .ok_or_else(|| "/proc/loadavg is empty".to_string())?
        .parse::<f64>()
        .map_err(|error| format!("invalid one-minute load average: {error}"))
}

fn tmp_stats(path: &Path, is_tmpfs: bool) -> Result<TmpStats, String> {
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
    let block_size = if stats.f_frsize == 0 {
        stats.f_bsize
    } else {
        stats.f_frsize
    };
    let total_bytes = block_size.saturating_mul(stats.f_blocks);
    let free_bytes = block_size
        .saturating_mul(stats.f_bavail)
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

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn memory_uses_available_memory_and_clamps_free_to_total() {
        let values = parse_meminfo("MemTotal: 2048 kB\nMemAvailable: 512 kB\nSwapTotal: 0 kB\n");
        let ram = memory_stats(
            required_kb(&values, "MemTotal").unwrap(),
            values["MemAvailable"],
        );
        assert_eq!((ram.total_mb, ram.used_mb, ram.free_mb), (2, 1, 0));
        assert_eq!(ram.used_percent, 75.0);
        let zero = memory_stats(0, 20);
        assert_eq!(zero.used_percent, 0.0);
    }

    #[test]
    fn mounts_choose_the_longest_decoded_mountpoint() {
        let mounts = "root / ext4 rw 0 0\ntmpfs /tmp tmpfs rw 0 0\n";
        assert_eq!(mount_type_for(mounts, "/tmp").as_deref(), Some("tmpfs"));
        assert_eq!(
            mount_type_for(mounts, "/tmp/subdir").as_deref(),
            Some("tmpfs")
        );
        assert_eq!(mount_type_for(mounts, "/tmpish").as_deref(), Some("ext4"));
        assert_eq!(decode_mount_field("/a\\040b"), "/a b");
    }

    #[test]
    fn load_parser_reads_one_minute_average_and_rejects_invalid_input() {
        assert_eq!(parse_load_one("1.25 0.50 0.10 2/44 123").unwrap(), 1.25);
        assert!(parse_load_one("bad").is_err());
    }
}
