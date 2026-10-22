//! Snapshot `~/.orquestra` (configs, presets, vault, projections) into a
//! `tar.gz` under `~/.orquestra/backups/`, excluding previous backups.
//!
//! The archive embeds `vault.json` with cleartext keys, so both the archive
//! (`0600`) and the `backups/` directory itself (`0700`) are locked down to
//! the owner right after creation. The inherited umask (`0644` on most
//! systems) would otherwise leave secrets readable by any local user.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Build `orquestra_backup_<stamp>.tar.gz` inside `root/backups/` and lock
/// its permissions. Returns the archive path. `root` must already exist.
pub fn create_backup_at(root: &Path) -> Result<PathBuf, String> {
    let backups = root.join("backups");
    std::fs::create_dir_all(&backups)
        .map_err(|error| format!("failed to create {}: {error}", backups.display()))?;
    lock_dir(&backups)?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let archive = backups.join(format!("orquestra_backup_{stamp}.tar.gz"));

    // Arg vector, no shell: nothing here is ever interpreted.
    let status = std::process::Command::new("tar")
        .args([
            "-czf",
            archive.to_str().ok_or("Unrepresentable backup path")?,
            "-C",
            root.to_str().ok_or("Unrepresentable root path")?,
            "--exclude=backups",
            ".",
        ])
        .status()
        .map_err(|error| format!("Tar execution failed: {error}"))?;
    if !status.success() {
        return Err("Failed to create archive".to_string());
    }
    lock_file(&archive)?;

    Ok(archive)
}

/// Owner-only on the `backups/` directory: no listing or entry by group/other.
fn lock_dir(path: &Path) -> Result<(), String> {
    set_mode(path, 0o700, "backups directory")
}

/// Owner-only on the archive: the tarball embeds cleartext vault secrets.
fn lock_file(path: &Path) -> Result<(), String> {
    set_mode(path, 0o600, "backup archive")
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32, what: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| format!("failed to lock down {what} {}: {error}", path.display()))
}

/// Windows has no POSIX mode bits; the file stays in the user's profile and
/// inherits its ACLs. A no-op keeps the command compiling on all targets.
#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32, _what: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::create_backup_at;

    #[cfg(unix)]
    fn mode_of(path: &std::path::Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::symlink_metadata(path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    }

    fn sandbox(name: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::path::PathBuf::from("/var/tmp/testing")
            .join(format!("orquestra-backup-{name}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// The archive must be owner-only even though tar inherits the umask, and
    /// the `backups/` dir must tighten even when pre-created wide open.
    #[cfg(unix)]
    #[test]
    fn backup_archive_and_dir_are_owner_only() {
        let root = sandbox("perms");
        std::fs::write(root.join("vault.json"), "{\"k\":\"s3cr3t\"}").unwrap();
        // Simulate a backups dir left over from an older, unfixed version.
        let backups = root.join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&backups, std::fs::Permissions::from_mode(0o755)).unwrap();

        let archive = create_backup_at(&root).expect("backup succeeds");
        assert_eq!(mode_of(&archive), 0o600, "archive must be -rw-------");
        assert_eq!(mode_of(&backups), 0o700, "backups dir must be drwx------");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backup_fails_cleanly_when_backups_is_not_a_dir() {
        let root = sandbox("notadir");
        // A regular file where `backups/` should be: create_dir_all fails
        // and the error must name the path instead of panicking.
        std::fs::write(root.join("backups"), "blocking file").unwrap();
        let error = create_backup_at(&root).expect_err("backups is a file");
        assert!(error.contains("backups"), "unexpected error: {error}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
