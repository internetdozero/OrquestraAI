//! The Windows syscalls behind the health snapshot: memory, system times and
//! the per-process working set.
//!
//! Only compiled on Windows. Every buffer is allocated here and handed to the
//! API with its exact length, so a partially filled answer is never read past
//! what the kernel wrote.

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows_sys::Win32::System::ProcessStatus::{
    EnumProcesses, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::{
    GetSystemTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::super::{memory_stats, MemoryStats, TopProcess};
use super::{process_name_from_path, rank_by_rss, swap_stats};

/// RAM and the page-file-derived swap figures.
pub(super) fn memory() -> Result<(MemoryStats, MemoryStats), String> {
    // SAFETY: a zeroed `MEMORYSTATUSEX` is a valid input once `dwLength` is
    // set; the API needs no other initialization.
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: `status` is a live, correctly sized, properly aligned buffer.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return Err(format!(
            "GlobalMemoryStatusEx failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let ram = memory_stats(status.ullTotalPhys / 1024, status.ullAvailPhys / 1024);
    let (swap_total_kb, swap_free_kb) = swap_stats(
        status.ullTotalPhys,
        status.ullAvailPhys,
        status.ullTotalPageFile,
        status.ullAvailPageFile,
    );
    Ok((ram, memory_stats(swap_total_kb, swap_free_kb)))
}

/// Since-boot CPU utilization, built from the cumulative `GetSystemTimes`
/// counters.
pub(super) fn cpu_utilization() -> Result<f64, String> {
    let mut idle: FILETIME = zeroed_filetime();
    let mut kernel: FILETIME = zeroed_filetime();
    let mut user: FILETIME = zeroed_filetime();
    // SAFETY: three distinct, aligned output buffers owned by this frame.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return Err(format!(
            "GetSystemTimes failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(super::cpu_utilization(
        ticks(idle),
        ticks(kernel),
        ticks(user),
    ))
}

fn zeroed_filetime() -> FILETIME {
    // SAFETY: `FILETIME` is two `u32`s; zero is the documented "no time yet".
    unsafe { std::mem::zeroed() }
}

/// 100-ns ticks in a `FILETIME` as one `u64`.
fn ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

/// Heaviest working sets first. Processes this account cannot open (system
/// services, other sessions) are skipped rather than reported as zero.
pub(super) fn top_processes(limit: usize) -> Vec<TopProcess> {
    const INITIAL_CAPACITY: usize = 1024;
    let mut pids = vec![0u32; INITIAL_CAPACITY];
    let mut needed: u32 = 0;
    let bytes = (pids.len() * std::mem::size_of::<u32>()) as u32;
    // SAFETY: the buffer holds `bytes` writable bytes and `needed` is a valid
    // out pointer.
    if unsafe { EnumProcesses(pids.as_mut_ptr(), bytes, &mut needed) } == 0 {
        return Vec::new();
    }
    // A full buffer means the list was truncated: retry once with room, which
    // is enough for a desktop machine and never loops unboundedly.
    if needed as usize >= pids.len() * std::mem::size_of::<u32>() {
        let grown = (pids.len() * 2).max(needed as usize / 4 + 1);
        pids = vec![0u32; grown];
        let bytes = (pids.len() * std::mem::size_of::<u32>()) as u32;
        // SAFETY: same contract as above with the larger buffer.
        if unsafe { EnumProcesses(pids.as_mut_ptr(), bytes, &mut needed) } == 0 {
            return Vec::new();
        }
    }

    let count = (needed as usize / std::mem::size_of::<u32>()).min(pids.len());
    let processes = pids[..count]
        .iter()
        .filter_map(|pid| process_entry(*pid))
        .collect();
    rank_by_rss(processes, limit)
}

/// One process: pid, image name and resident megabytes.
fn process_entry(pid: u32) -> Option<TopProcess> {
    if pid == 0 {
        return None;
    }
    // SAFETY: `OpenProcess` takes plain scalars; the handle is checked below
    // and closed exactly once on the success path.
    let handle: HANDLE = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }

    let rss_mb = working_set_mb(handle)?;
    let name = image_name(handle).unwrap_or_else(|| format!("pid {pid}"));
    // SAFETY: `handle` came from `OpenProcess` above and is not used after.
    unsafe { CloseHandle(handle) };
    Some(TopProcess { pid, name, rss_mb })
}

fn working_set_mb(handle: HANDLE) -> Option<u64> {
    // SAFETY: zeroed counters with `cb` set are the documented input shape.
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: `handle` is open and `counters` is a live buffer of the size
    // passed as `cb`.
    let ok = unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    };
    if ok == 0 {
        return None;
    }
    Some((counters.WorkingSetSize as u64) / 1_048_576)
}

fn image_name(handle: HANDLE) -> Option<String> {
    let mut buffer = vec![0u16; 1024];
    let mut length = buffer.len() as u32;
    // SAFETY: `buffer` holds `length` writable UTF-16 units; the API writes at
    // most `length` units and reports how many it used.
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) };
    if ok == 0 || length == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    Some(process_name_from_path(&path))
}
