//! Backup run naming and empty-run cleanup.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use ownhub_core::paths::OwnHubPaths;

const BACKUP_PREFIX: &str = "pre-fusion";
const MAX_BACKUPS: usize = 3;

pub(super) fn next_backup_path(paths: &OwnHubPaths) -> PathBuf {
    let backups = paths.root.join("backups");
    let base = format!("{BACKUP_PREFIX}-{}", super::timestamp_label());
    let mut candidate = backups.join(&base);
    let mut suffix = 1usize;
    while candidate.exists() {
        candidate = backups.join(format!("{base}-{suffix}"));
        suffix += 1;
    }
    candidate
}

pub(super) fn finalize_backup(path: PathBuf) -> Result<Option<PathBuf>, String> {
    match fs::read_dir(&path) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                fs::remove_dir(&path).map_err(|error| {
                    format!("failed to remove empty backup {}: {error}", path.display())
                })?;
                Ok(None)
            } else {
                Ok(Some(path))
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Ok(Some(path)),
    }
}

/// Keep the three newest bridge backups, pruning only completed `pre-fusion-*`
/// directories. Call after a successful fusion so a failed/incomplete run never
/// causes older recovery points to be discarded.
pub(super) fn retain_recent_backups(backups_dir: &Path) -> Result<(), String> {
    let entries = match fs::read_dir(backups_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "failed to list backups {}: {error}",
                backups_dir.display()
            ))
        }
    };

    let mut backups = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read backup entry in {}: {error}",
                backups_dir.display()
            )
        })?;
        let name = entry.file_name();
        let Some(order) = backup_order(&name.to_string_lossy()) else {
            continue;
        };

        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
            format!(
                "failed to inspect backup {}: {error}",
                entry.path().display()
            )
        })?;
        // Do not follow symlinks or touch files that happen to share the prefix.
        if metadata.file_type().is_dir() {
            backups.push((order, entry.path()));
        }
    }

    backups.sort_by(|left, right| left.0.cmp(&right.0));
    let remove_count = backups.len().saturating_sub(MAX_BACKUPS);
    for (_, path) in backups.into_iter().take(remove_count) {
        fs::remove_dir_all(&path)
            .map_err(|error| format!("failed to prune old backup {}: {error}", path.display()))?;
    }

    Ok(())
}

/// Sort key for names produced by `next_backup_path`: sortable timestamp, then
/// numeric collision suffix (the un-suffixed name is the first run that second).
fn backup_order(name: &str) -> Option<(String, u64)> {
    let rest = name.strip_prefix(&format!("{BACKUP_PREFIX}-"))?;
    let bytes = rest.as_bytes();
    if bytes.len() < 15 {
        return None;
    }

    let timestamp = std::str::from_utf8(&bytes[..15]).ok()?;
    let valid_timestamp = timestamp.as_bytes()[8] == b'-'
        && timestamp.as_bytes()[..8]
            .iter()
            .all(|byte| byte.is_ascii_digit())
        && timestamp.as_bytes()[9..]
            .iter()
            .all(|byte| byte.is_ascii_digit());
    if !valid_timestamp {
        return None;
    }

    let suffix = match bytes.get(15..) {
        None | Some([]) => 0,
        Some([b'-', digits @ ..])
            if !digits.is_empty() && digits.iter().all(u8::is_ascii_digit) =>
        {
            std::str::from_utf8(digits).ok()?.parse().ok()?
        }
        _ => return None,
    };

    Some((timestamp.to_string(), suffix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("ownhub-backup-retention-{unique}"))
    }

    #[test]
    fn retains_three_newest_bridge_backups_and_leaves_other_entries_alone() {
        let root = scratch();
        fs::create_dir_all(&root).expect("backup root");
        for name in [
            "pre-fusion-20260101-000000",
            "pre-fusion-20260102-000000",
            "pre-fusion-20260103-000000",
            "pre-fusion-20260104-000000",
            "pre-fusion-20260104-000000-2",
            "pre-fusion-20260104-000000-10",
        ] {
            let backup = root.join(name);
            fs::create_dir_all(&backup).expect("backup dir");
            fs::write(backup.join("saved.txt"), name).expect("backup contents");
        }
        let unrelated = root.join("desktop-backup-20260101");
        fs::create_dir_all(&unrelated).expect("unrelated dir");
        fs::write(unrelated.join("keep.txt"), "keep").expect("unrelated contents");
        let same_prefix_file = root.join("pre-fusion-not-a-directory");
        fs::write(&same_prefix_file, "keep").expect("prefixed file");

        retain_recent_backups(&root).expect("rotation");

        for name in [
            "pre-fusion-20260101-000000",
            "pre-fusion-20260102-000000",
            "pre-fusion-20260103-000000",
        ] {
            assert!(
                !root.join(name).exists(),
                "old backup {name} must be pruned"
            );
        }
        for name in [
            "pre-fusion-20260104-000000",
            "pre-fusion-20260104-000000-2",
            "pre-fusion-20260104-000000-10",
        ] {
            assert!(root.join(name).join("saved.txt").is_file(), "{name}");
        }
        assert!(unrelated.join("keep.txt").is_file());
        assert!(same_prefix_file.is_file());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn rotation_never_follows_a_prefixed_symlink() {
        let root = scratch();
        let outside = scratch();
        fs::create_dir_all(&root).expect("backup root");
        fs::create_dir_all(&outside).expect("outside");
        fs::write(outside.join("keep.txt"), "keep").expect("outside contents");
        std::os::unix::fs::symlink(&outside, root.join("pre-fusion-20260101-000000"))
            .expect("backup symlink");

        retain_recent_backups(&root).expect("rotation");

        assert!(outside.join("keep.txt").is_file());
        assert!(root.join("pre-fusion-20260101-000000").is_symlink());
        fs::remove_dir_all(root).expect("cleanup root");
        fs::remove_dir_all(outside).expect("cleanup outside");
    }
}
