//! Darwin collection: sysctl, Mach VM statistics and `/bin/ps`.
//!
//! Nothing here reads `/proc`. Memory comes from `host_statistics64` page
//! counters, swap from `vm.swapusage`, load from `getloadavg`, and capacity
//! from `statvfs` on the volume that actually holds the scratch root (macOS
//! has no tmpfs, so `is_tmpfs` is always false).
//!
//! Top processes shell out to `/bin/ps` rather than calling libproc:
//! `proc_pid_rusage` returns `rusage_info_v2`, whose layout would have to be
//! mirrored by hand with fixed offsets, and a wrong offset is silent memory
//! corruption instead of a compile error. `ps` is part of the base system and
//! its output is parsed by a function the Linux CI can test.

use std::ffi::{c_void, CString};
use std::path::PathBuf;
use std::sync::OnceLock;

use super::{darwin_memory_stats, parse_ps, DarwinMemoryCounts};
use crate::janitor::health::{
    memory_stats, should_alert, tmp_stats, CpuStats, MemoryStats, SystemHealth, TopProcess,
};

pub(super) fn snapshot() -> Result<SystemHealth, String> {
    let page_size = sysctl_u64("hw.pagesize")?.max(1);
    let total_bytes = sysctl_u64("hw.memsize")?;
    let ram = darwin_memory_stats(total_bytes, page_size, vm_counters()?);
    let swap = swap_stats()?;
    let cores = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .max(1);
    let cpu = CpuStats {
        cores,
        load_percent: load_average()? * 100.0 / cores as f64,
    };
    let tmp = tmp_stats(&scratch_volume(), false)?;
    let alert = should_alert(ram.used_percent, tmp.used_percent);
    Ok(SystemHealth {
        ram,
        swap,
        tmp,
        cpu,
        top_processes: top_processes(5),
        alert,
    })
}

/// The volume holding the scratch root. `scratch_root()` is created on demand,
/// so walk up to the nearest ancestor that exists: `/var/folders/...` by
/// default, `$XDG_RUNTIME_DIR` when the operator set one.
fn scratch_volume() -> PathBuf {
    let root = crate::paths::scratch_root();
    let mut probe = root.as_path();
    loop {
        if probe.exists() {
            return probe.to_path_buf();
        }
        match probe.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => probe = parent,
            _ => return PathBuf::from("/"),
        }
    }
}

/// Read a sysctl by name. The kernel reports its size first, so a future
/// version that grows a struct is rejected by length instead of over-read.
fn sysctl_raw(name: &str) -> Result<Vec<u8>, String> {
    let key = CString::new(name).map_err(|_| format!("sysctl name {name} contains a NUL byte"))?;
    let mut size: libc::size_t = 0;
    // SAFETY: a NULL `oldp` with a valid `oldlenp` is the documented size
    // query of sysctl(3); both pointers outlive the call.
    let probe = unsafe {
        libc::sysctlbyname(
            key.as_ptr(),
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if probe != 0 {
        return Err(format!(
            "could not size sysctl {name}: {}",
            std::io::Error::last_os_error()
        ));
    }
    if size == 0 {
        return Err(format!("sysctl {name} returned no bytes"));
    }
    let mut buffer = vec![0u8; size];
    // SAFETY: `buffer` holds exactly `size` writable bytes and `size` still
    // carries the kernel-reported length.
    let read = unsafe {
        libc::sysctlbyname(
            key.as_ptr(),
            buffer.as_mut_ptr().cast::<c_void>(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 {
        return Err(format!(
            "could not read sysctl {name}: {}",
            std::io::Error::last_os_error()
        ));
    }
    buffer.truncate(size);
    Ok(buffer)
}

fn sysctl_u64(name: &str) -> Result<u64, String> {
    let bytes = sysctl_raw(name)?;
    if bytes.len() != std::mem::size_of::<u64>() {
        return Err(format!(
            "sysctl {name} returned {} bytes, expected 8",
            bytes.len()
        ));
    }
    let mut value = [0u8; 8];
    value.copy_from_slice(&bytes);
    Ok(u64::from_ne_bytes(value))
}

fn vm_counters() -> Result<DarwinMemoryCounts, String> {
    let mut stats = std::mem::MaybeUninit::<libc::vm_statistics64_data_t>::zeroed();
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: `stats` is sized for exactly HOST_VM_INFO64_COUNT integers, so
    // the kernel writes at most that many into it.
    let result = unsafe {
        libc::host_statistics64(
            host_port(),
            libc::HOST_VM_INFO64,
            stats.as_mut_ptr().cast::<libc::integer_t>(),
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return Err(format!("host_statistics64 failed (kern_return_t {result})"));
    }
    // SAFETY: a KERN_SUCCESS return means the structure was filled in.
    let stats = unsafe { stats.assume_init() };
    Ok(DarwinMemoryCounts {
        free: u64::from(stats.free_count),
        speculative: u64::from(stats.speculative_count),
        active: u64::from(stats.active_count),
        wired: u64::from(stats.wire_count),
        compressor: u64::from(stats.compressor_page_count),
    })
}

/// `mach_host_self()` adds a send right on every call, so take the port once
/// per process instead of leaking a reference on every 5-second refresh.
///
/// `libc` marks this binding deprecated in favour of the separate `mach2`
/// crate; the underlying call is still the documented way to name the host
/// port, and a new dependency for one call would not pay for itself.
#[allow(deprecated)]
fn host_port() -> libc::host_t {
    static HOST: OnceLock<libc::host_t> = OnceLock::new();
    // SAFETY: `mach_host_self` takes no arguments and returns the host port
    // for this task; it has no failure mode.
    *HOST.get_or_init(|| unsafe { libc::mach_host_self() })
}

fn swap_stats() -> Result<MemoryStats, String> {
    let bytes = sysctl_raw("vm.swapusage")?;
    let expected = std::mem::size_of::<libc::xsw_usage>();
    if bytes.len() < expected {
        return Err(format!(
            "sysctl vm.swapusage returned {} bytes, expected at least {expected}",
            bytes.len()
        ));
    }
    // SAFETY: read_unaligned over a buffer at least `size_of::<xsw_usage>()`
    // long that the kernel filled with exactly that struct.
    let usage = unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<libc::xsw_usage>()) };
    Ok(memory_stats(usage.xsu_total / 1024, usage.xsu_avail / 1024))
}

fn load_average() -> Result<f64, String> {
    let mut loads = [0f64; 1];
    // SAFETY: one element requested, one element of storage provided.
    let read = unsafe { libc::getloadavg(loads.as_mut_ptr(), 1) };
    if read != 1 {
        return Err(format!("getloadavg returned {read}, expected 1"));
    }
    Ok(loads[0])
}

/// Best-effort top RSS consumers. Metrics degrade, they never fail the
/// snapshot: an empty list is what the panel already renders when a platform
/// exposes nothing.
fn top_processes(limit: usize) -> Vec<TopProcess> {
    let output = match std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,rss=,comm="])
        .env("LC_ALL", "C")
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    let mut processes = parse_ps(&String::from_utf8_lossy(&output.stdout));
    processes.truncate(limit);
    processes
}
