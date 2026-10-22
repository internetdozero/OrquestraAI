use serde::Serialize;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;
use std::time::{Duration, SystemTime};

const SCRATCH_MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepReport {
    pub reclaimed_mb: f64,
    pub removed_count: usize,
    pub status: String,
}

pub(super) fn sweep_at(
    orquestra_root: &Path,
    scratch_root: &Path,
    active_sockets: &str,
    now: SystemTime,
) -> Result<SweepReport, String> {
    let mut report = SweepReport {
        reclaimed_mb: 0.0,
        removed_count: 0,
        status: String::new(),
    };
    clean_sockets(orquestra_root, active_sockets, &mut report)?;
    clean_scratch(scratch_root, now, &mut report)?;
    report.reclaimed_mb = (report.reclaimed_mb * 10.0).round() / 10.0;
    report.status = if report.removed_count == 0 {
        "Sweep completed; no stale files found.".to_string()
    } else {
        "Sweep completed.".to_string()
    };
    Ok(report)
}

/// No Unix sockets exist off Unix, and liveness comes from
/// `/proc/net/unix` — fail closed by sweeping nothing.
#[cfg(not(unix))]
fn clean_sockets(_root: &Path, _active: &str, _report: &mut SweepReport) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn clean_sockets(root: &Path, active: &str, report: &mut SweepReport) -> Result<(), String> {
    if !is_real_directory(root)? {
        return Ok(());
    }
    let entries = fs::read_dir(root)
        .map_err(|error| format!("could not inspect {}: {error}", root.display()))?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry.file_name().to_string_lossy().ends_with(".sock") {
            continue;
        }
        let path = entry.path();
        let before = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if !before.file_type().is_socket() || socket_is_active(active, &path) {
            continue;
        }
        let after = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if !same_file(&before, &after) || socket_is_active(active, &path) {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => account(report, before.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
    Ok(())
}

fn clean_scratch(root: &Path, now: SystemTime, report: &mut SweepReport) -> Result<(), String> {
    if !is_real_directory(root)? {
        return Ok(());
    }
    let entries = fs::read_dir(root)
        .map_err(|error| format!("could not inspect {}: {error}", root.display()))?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        let before = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if !before.file_type().is_file() || !is_expired(&before, now) {
            continue;
        }
        let after = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if !same_file(&before, &after) || !after.file_type().is_file() || !is_expired(&after, now) {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => account(report, before.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
    Ok(())
}

fn is_real_directory(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_dir()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("could not inspect {}: {error}", path.display())),
    }
}

#[cfg(unix)]
fn socket_is_active(proc_net_unix: &str, path: &Path) -> bool {
    let wanted = path.to_string_lossy();
    proc_net_unix.lines().skip(1).any(|line| {
        let mut remaining = line;
        for _ in 0..7 {
            let trimmed = remaining.trim_start();
            let Some(end) = trimmed.find(char::is_whitespace) else {
                return false;
            };
            remaining = &trimmed[end..];
        }
        remaining.trim() == wanted
    })
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

/// Fallback identity where dev/ino do not exist: length plus mtime.
/// Weaker than (dev, ino), but this only guards a best-effort sweep.
#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn is_expired(metadata: &fs::Metadata, now: SystemTime) -> bool {
    metadata
        .modified()
        .ok()
        .and_then(|modified| now.duration_since(modified).ok())
        .is_some_and(|age| age > SCRATCH_MAX_AGE)
}

fn account(report: &mut SweepReport, bytes: u64) {
    report.removed_count += 1;
    report.reclaimed_mb += bytes as f64 / 1_048_576.0;
}
