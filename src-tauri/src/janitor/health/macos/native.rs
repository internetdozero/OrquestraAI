//! Darwin collection: sysctl, Mach VM statistics and libproc.
//!
//! Nothing here reads `/proc` and nothing shells out. Memory comes from
//! `host_statistics64` page counters, swap from `vm.swapusage`, load from
//! `getloadavg`, capacity from `statvfs` on the volume that actually holds the
//! scratch root (macOS has no tmpfs, so `is_tmpfs` is always false), and the
//! top processes from `proc_listpids` + `proc_taskinfo` + `proc_name`.
//!
//! The kernel writes the whole `proc_taskinfo`, so the whole struct is asked
//! for; only its first two fields (`pti_virtual_size`, then `pti_resident_size`)
//! are ever read, and only when the kernel reported writing those 16 bytes. No
//! later field can shift what this code trusts, and a short read is skipped
//! instead of misread.

use std::ffi::{c_int, c_void, CString};
use std::path::PathBuf;
use std::sync::OnceLock;

use super::{darwin_memory_stats, process_name, rank_by_rss, resident_bytes, DarwinMemoryCounts};
use crate::janitor::health::{
    memory_stats, should_alert, tmp_stats, CpuStats, MemoryStats, SystemHealth, TopProcess,
};

/// `PROC_ALL_PIDS` from `<libproc.h>`. The pid list itself, `PROC_PIDTASKINFO`
/// and `proc_name` all come from `libc`'s macOS-gated libproc bindings, so
/// this module declares no FFI of its own and links nothing by hand.
const PROC_ALL_PIDS: u32 = 1;
/// `PROC_PIDTASKINFO` from `<libproc.h>`.
const PROC_PIDTASKINFO: c_int = 4;
/// Two `u64`s: virtual size then resident size.
const LEADING_TASKINFO_BYTES: usize = 16;
/// Size of the struct the kernel writes for `PROC_PIDTASKINFO`, taken from
/// libc's own definition so no byte count is hardcoded here. Only
/// [`LEADING_TASKINFO_BYTES`] of the result are ever read.
const TASKINFO_BYTES: usize = std::mem::size_of::<libc::proc_taskinfo>();
/// The slice [`resident_bytes`] reads must be `pti_resident_size`, not a
/// neighbouring field. Checked while compiling for Darwin, so a libc that
/// reorders the struct fails the build instead of reporting wrong numbers.
const _: () = assert!(std::mem::offset_of!(libc::proc_taskinfo, pti_resident_size) == 8);
const _: () = assert!(std::mem::offset_of!(libc::proc_taskinfo, pti_virtual_size) == 0);
/// `proc_name` truncates to `PROC_PIDPATHINFO_MAXSIZE`; 256 covers a basename
/// with room to spare.
const NAME_BUFFER: usize = 256;
/// Slack for processes that start between the size query and the read.
const PID_SLACK: i32 = 128;
/// Refuse to allocate more than 1 MiB of pid list, whatever the kernel says.
const MAX_PID_BYTES: i32 = 1 << 20;

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

/// Every pid the kernel reports. The size query answers with either a byte
/// count or a pid count depending on the release, so the allocation is
/// `(answer + slack) * size_of::<pid_t>()` — enough under both conventions —
/// and the second call's byte count decides how many entries are real.
fn all_pids() -> Vec<libc::pid_t> {
    // SAFETY: a NULL buffer with size 0 is the documented size query.
    let hint = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    if hint <= 0 {
        return Vec::new();
    }
    let bytes = hint
        .saturating_add(PID_SLACK)
        .saturating_mul(std::mem::size_of::<libc::pid_t>() as i32)
        .min(MAX_PID_BYTES);
    let mut buffer: Vec<libc::pid_t> = vec![0; bytes as usize / std::mem::size_of::<libc::pid_t>()];

    // SAFETY: `buffer` holds `bytes` writable bytes.
    let written = unsafe {
        libc::proc_listpids(
            PROC_ALL_PIDS,
            0,
            buffer.as_mut_ptr().cast::<c_void>(),
            bytes,
        )
    };
    if written <= 0 {
        return Vec::new();
    }
    buffer.truncate(written as usize / std::mem::size_of::<libc::pid_t>());
    buffer.retain(|pid| *pid > 0);
    buffer
}

/// Resident bytes for one process, or `None` when the kernel refused the pid
/// or wrote fewer than the two leading `u64`s.
fn resident_of(pid: libc::pid_t) -> Option<u64> {
    let mut buffer = [0u8; TASKINFO_BYTES];
    // SAFETY: `buffer` is TASKINFO_BYTES writable bytes and the call reports
    // how many it wrote; the length is checked before it is read.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            PROC_PIDTASKINFO,
            0,
            buffer.as_mut_ptr().cast::<c_void>(),
            TASKINFO_BYTES as c_int,
        )
    };
    if written < LEADING_TASKINFO_BYTES as c_int {
        return None;
    }
    let filled = (written as usize).min(TASKINFO_BYTES);
    resident_bytes(&buffer[..filled])
}

/// Basename of a process, or `None` when `proc_name` had nothing to say
/// (typically a pid that exited or belongs to another user's protected task).
fn name_of(pid: libc::pid_t) -> Option<String> {
    let mut buffer = [0u8; NAME_BUFFER];
    // SAFETY: `buffer` is NAME_BUFFER writable bytes and `proc_name` writes at
    // most that many.
    let written = unsafe {
        libc::proc_name(
            pid,
            buffer.as_mut_ptr().cast::<c_void>(),
            NAME_BUFFER as u32,
        )
    };
    if written <= 0 {
        return None;
    }
    let filled = (written as usize).min(NAME_BUFFER);
    process_name(&buffer[..filled])
}

/// Top processes by resident size, system-wide. Unlike the Linux collector
/// this is not limited to the operator's own processes: libproc reports a
/// uid only through a much larger struct, and the panel is a diagnostic —
/// the busiest processes on the machine are the interesting ones.
fn top_processes(limit: usize) -> Vec<TopProcess> {
    let mut processes = Vec::new();
    for pid in all_pids() {
        let Some(resident) = resident_of(pid) else {
            continue;
        };
        let Some(name) = name_of(pid) else {
            continue;
        };
        processes.push(TopProcess {
            pid: pid as u32,
            name,
            rss_mb: resident / 1_048_576,
        });
    }
    rank_by_rss(&mut processes, limit);
    processes
}
