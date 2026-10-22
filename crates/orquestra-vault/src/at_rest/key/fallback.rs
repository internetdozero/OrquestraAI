//! The `0600` fallback key file used when no OS keyring is available.

use std::path::{Path, PathBuf};

use base64::prelude::*;

use crate::at_rest::io::zeroize_file_contents;

use super::fresh_key;

const FALLBACK_FILE: &str = ".file-key";

pub(crate) fn fallback_path(root: &Path) -> PathBuf {
    root.join(FALLBACK_FILE)
}

pub(crate) fn fallback_exists(root: &Path) -> bool {
    fallback_path(root).is_file()
}

/// Read-only fallback lookup. Never creates: `None` means no key file.
/// An existing file with the wrong mode/owner or bad contents fails closed.
pub(crate) fn read_fallback_key(root: &Path) -> Result<Option<[u8; 32]>, String> {
    let path = fallback_path(root);
    let stored = match std::fs::read_to_string(&path) {
        Ok(stored) => stored,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    verify_fallback_file(&path)?;
    BASE64_STANDARD
        .decode(stored.trim())
        .map_err(|error| format!("key file {} is corrupt: {error}", path.display()))
        .and_then(|raw| {
            raw.try_into()
                .map_err(|_| format!("key file {} has the wrong length", path.display()))
        })
        .map(Some)
}

/// The fallback key file must be exactly `0600` and owned by this uid: a
/// group/world-readable fallback voids the encryption entirely.
fn verify_fallback_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        let mode = metadata.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(format!(
                "refusing to use {}: mode is {mode:04o}, expected 0600",
                path.display()
            ));
        }
        // SAFETY: `getuid` takes no arguments and has no failure mode.
        let uid = unsafe { libc::getuid() };
        if metadata.uid() != uid {
            return Err(format!(
                "refusing to use {}: not owned by the current user",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Create the fallback key file atomically (`create_new`, `0600` at
/// creation — no write-then-chmod window). A racy second creator falls back
/// to reading the winner's file.
pub(crate) fn create_fallback_key(root: &Path) -> Result<[u8; 32], String> {
    let path = fallback_path(root);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    let key = fresh_key()?;
    let content = format!("{}\n", BASE64_STANDARD.encode(key));
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => {
                use std::io::Write;
                let mut file = file;
                file.write_all(content.as_bytes())
                    .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
                file.sync_all()
                    .map_err(|error| format!("failed to sync {}: {error}", path.display()))?;
                verify_fallback_file(&path)?;
                Ok(key)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                read_fallback_key(root)?.ok_or_else(|| {
                    format!("key file {} appeared but does not open", path.display())
                })
            }
            Err(error) => Err(format!("failed to create {}: {error}", path.display())),
        }
    }
    #[cfg(not(unix))]
    {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => {
                std::fs::write(&path, content)
                    .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
                Ok(key)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                read_fallback_key(root)?.ok_or_else(|| {
                    format!("key file {} appeared but does not open", path.display())
                })
            }
            Err(error) => Err(format!("failed to create {}: {error}", path.display())),
        }
    }
}

/// Remove the fallback key file (zeroize + unlink). Returns `false` when
/// absent. Best effort on SSDs/CoW — see `crate::at_rest::purge_migration_backup`.
pub(crate) fn remove_fallback_key(root: &Path) -> Result<bool, String> {
    let path = fallback_path(root);
    let metadata = match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", path.display()));
        }
        Ok(metadata) => metadata,
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "refusing to remove {}: not a regular file",
            path.display()
        ));
    }
    zeroize_file_contents(&path, metadata.len())?;
    std::fs::remove_file(&path)
        .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
    Ok(true)
}
