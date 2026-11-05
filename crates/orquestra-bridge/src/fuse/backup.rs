//! Backup run naming and empty-run cleanup.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use orquestra_core::paths::OrquestraPaths;

const BACKUP_PREFIX: &str = "pre-fusion";

pub(super) fn next_backup_path(paths: &OrquestraPaths) -> PathBuf {
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
