//! Installing a downloaded binary over the running one.
//!
//! The two platforms disagree about what is possible, so the difference lives
//! here instead of inside `apply_app_update`.

use std::fs;
use std::path::{Path, PathBuf};

/// Install `staged` at `target`, which is the running image.
///
/// Unix renames over the live path: the kernel keeps the old inode alive for
/// the running process and the next launch sees the new file.
///
/// Windows cannot delete or overwrite a running image (`ERROR_ACCESS_DENIED`,
/// the "sharing violation" an in-place rename reports), but it *can* rename
/// it. So the live file is retired under a dotted sibling name, the staged
/// file takes its place, and the leftover is deleted on the next launch by
/// [`purge_retired_binaries`]. If the second rename fails, the original is put
/// back so a failed update never leaves the app unlaunchable.
pub(super) fn install(staged: &Path, target: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let retired = retired_binary_path(target);
        fs::rename(target, &retired)
            .map_err(|error| format!("failed to retire {}: {error}", target.display()))?;
        if let Err(error) = fs::rename(staged, target) {
            let _ = fs::rename(&retired, target);
            return Err(format!("failed to install {}: {error}", target.display()));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        fs::rename(staged, target)
            .map_err(|error| format!("failed to replace {}: {error}", target.display()))
    }
}

/// Dotted sibling name a replaced image is parked under. Windows retires the
/// running image; the test pins the naming on every host.
#[cfg(any(windows, test))]
pub(super) fn retired_binary_path(target: &Path) -> PathBuf {
    target.with_file_name(format!(
        ".{}.old-{}",
        target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "ownhub".to_string()),
        crate::paths::unique_tmp_suffix()
    ))
}

/// Delete retired images left next to the running binary. Best-effort: a
/// still-locked file simply stays until the next launch.
pub fn purge_retired_binaries() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(directory) = exe.parent() else {
        return;
    };
    purge_retired_binaries_in(directory);
}

fn purge_retired_binaries_in(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && name.contains(".old-") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Running executable first, `~/.local/bin/ownhub` as fallback.
pub(super) fn active_binary() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if exe.is_file() {
            return Ok(exe);
        }
    }
    let home = crate::paths::require_home("locate the active binary")?;
    let fallback = home.join(".local/bin/ownhub");
    if fallback.is_file() {
        return Ok(fallback);
    }
    Err(
        "cannot locate the running binary (current_exe and ~/.local/bin/ownhub both missing)"
            .to_string(),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// The retired image sits beside the target under a dotted name, so the
    /// purge prefix scan can find it without touching real binaries.
    #[test]
    fn retired_images_are_dotted_siblings_and_purged() {
        let dir = std::env::temp_dir().join(format!(
            "ownhub-update-purge-{}-{}",
            std::process::id(),
            crate::paths::unique_tmp_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let target = dir.join("ownhub");
        let retired = retired_binary_path(&target);
        assert_eq!(retired.parent(), Some(dir.as_path()));
        let name = retired.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with(".ownhub.old-"), "unexpected name {name}");

        std::fs::write(&retired, b"old").unwrap();
        std::fs::write(&target, b"new").unwrap();
        purge_retired_binaries_in(&dir);

        assert!(!retired.exists(), "the retired image must be removed");
        assert!(target.is_file(), "the live binary must survive the purge");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
