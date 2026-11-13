//! Snapshot `~/.orquestra` (configs, presets, vault, projections) into a
//! `tar.gz` under `~/.orquestra/backups/`, excluding previous backups.
//!
//! The archive embeds `vault.json` with cleartext keys, so the file is created
//! `0600` **before** a single byte is written — an inherited umask of `022`
//! would otherwise leave it readable by every local user for the whole time it
//! is being filled. The `backups/` directory is tightened to `0700` too.
//!
//! Archiving happens in-process (`tar` + `flate2`) instead of shelling out to
//! the system `tar`: Windows has no `tar.exe` on older builds, BSD tar and GNU
//! tar disagree about flags, and an external process would still have to be
//! probed at boot. Symlinks are stored as links, matching what the previous
//! command did, so projections stay small instead of duplicating the vault.
//!
//! Windows has no POSIX mode bits: the archive stays in the operator's profile
//! and inherits its ACL, which grants the owning account and Administrators
//! only. See `docs/security.md`.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Name of the backup directory inside `root`, excluded from its own archive.
const BACKUPS_DIR: &str = "backups";

/// Build `orquestra_backup_<stamp>.tar.gz` inside `root/backups/` and lock
/// its permissions. Returns the archive path. `root` must already exist.
pub fn create_backup_at(root: &Path) -> Result<PathBuf, String> {
    let backups = root.join(BACKUPS_DIR);
    std::fs::create_dir_all(&backups)
        .map_err(|error| format!("failed to create {}: {error}", backups.display()))?;
    lock_dir(&backups)?;

    let archive = backups.join(format!("orquestra_backup_{stamp}.tar.gz", stamp = stamp()));
    write_archive(root, &archive)?;
    lock_file(&archive)?;

    Ok(archive)
}

fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Write the archive, newest-first ordering not required but name-sorted for a
/// reproducible listing. A failure removes the partial file: a half-written
/// tarball that looks like a backup is worse than none.
fn write_archive(root: &Path, archive: &Path) -> Result<(), String> {
    let file = create_private_file(archive)
        .map_err(|error| format!("failed to create {}: {error}", archive.display()))?;

    let result = fill_archive(root, file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(archive);
        return Err(error);
    }
    Ok(())
}

fn fill_archive(root: &Path, file: File) -> Result<(), String> {
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    // Store projections as links, as `tar -czf` did: following them would
    // duplicate the whole vault into the archive.
    builder.follow_symlinks(false);

    let mut names: Vec<String> = std::fs::read_dir(root)
        .map_err(|error| format!("failed to read {}: {error}", root.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != BACKUPS_DIR)
        .collect();
    names.sort();

    for name in names {
        let path = root.join(&name);
        // `append_dir_all` needs a directory; top-level files (`vault.json`,
        // `AGENTS.md`) are appended by path, symlinks included as links.
        let is_dir = std::fs::symlink_metadata(&path)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false);
        let appended = if is_dir {
            builder.append_dir_all(&name, &path)
        } else {
            builder.append_path_with_name(&path, &name)
        };
        appended.map_err(|error| format!("failed to archive {name}: {error}"))?;
    }

    let encoder = builder
        .into_inner()
        .map_err(|error| format!("failed to finish the archive: {error}"))?;
    let mut file = encoder
        .finish()
        .map_err(|error| format!("failed to compress the archive: {error}"))?;
    file.flush()
        .map_err(|error| format!("failed to flush the archive: {error}"))?;
    // Durability: a backup that is still in the page cache when the machine
    // loses power is not a backup. `sync_all` also surfaces a full disk here
    // instead of at the next boot.
    file.sync_all()
        .map_err(|error| format!("failed to sync the archive: {error}"))
}

/// Create the archive exclusively with owner-only permissions from the start.
///
/// `create_new` refuses to reuse an existing name, so a same-second repeat
/// fails loudly instead of writing into a file someone else prepared.
fn create_private_file(path: &Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Still masked by the umask, which can only remove bits; `lock_file`
        // then sets the final mode explicitly.
        options.mode(0o600);
    }
    options.open(path)
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
/// inherits its ACL (owner and Administrators only). See `docs/security.md`.
#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32, _what: &str) -> Result<(), String> {
    Ok(())
}

/// Archive `root/name` into `archive` (`.tar.gz`), replacing any existing
/// file there. Used by `skills export --tar` so it never shells out to the
/// system `tar` (missing before Windows 10/1803, absent on minimal images).
pub fn write_dir_archive(root: &Path, name: &str, archive: &Path) -> Result<(), String> {
    let source = root.join(name);
    if !source.is_dir() {
        return Err(format!("{} is not a directory", source.display()));
    }

    let file = File::create(archive)
        .map_err(|error| format!("failed to create {}: {error}", archive.display()))?;
    let result = fill_dir_archive(&source, name, file);
    if result.is_err() {
        let _ = std::fs::remove_file(archive);
    }
    result.map_err(|error| format!("failed to archive {name}: {error}"))
}

fn fill_dir_archive(source: &Path, name: &str, file: File) -> Result<(), String> {
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    // Projections and skill folders may contain links; storing them as links
    // keeps the archive small and never duplicates content outside the vault.
    builder.follow_symlinks(false);
    builder
        .append_dir_all(name, source)
        .map_err(|error| error.to_string())?;
    let mut file = builder
        .into_inner()
        .map_err(|error| error.to_string())?
        .finish()
        .map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{create_backup_at, write_dir_archive};

    #[test]
    fn exports_one_directory_without_external_tar() {
        let scratch = crate::paths::fresh_scratch_dir("archive-export").expect("scratch");
        let skill = scratch.join("vault/demo-skill");
        std::fs::create_dir_all(&skill).expect("skill dir");
        std::fs::write(skill.join("SKILL.md"), "# demo\n").expect("skill file");

        let archive = scratch.join("demo-skill.tar.gz");
        write_dir_archive(&scratch.join("vault"), "demo-skill", &archive).expect("archive");

        let names = entry_names(&archive);
        assert!(
            names.iter().any(|name| name == "demo-skill/SKILL.md"),
            "{names:?}"
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }

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
        let root = std::env::temp_dir().join(format!(
            "orquestra-backup-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// Every entry name inside a backup, as `tar` stored it.
    fn entry_names(archive: &std::path::Path) -> Vec<String> {
        let file = std::fs::File::open(archive).unwrap();
        let decoder = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        let mut names = Vec::new();
        for entry in tar.entries().unwrap() {
            let entry = entry.unwrap();
            names.push(entry.path().unwrap().to_string_lossy().into_owned());
        }
        names
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

    /// The snapshot holds the configs but never a previous backup.
    #[test]
    fn archive_carries_the_store_without_old_backups() {
        let root = sandbox("contents");
        std::fs::write(root.join("vault.json"), "{\"k\":\"v\"}").unwrap();
        std::fs::create_dir_all(root.join("active/global")).unwrap();
        std::fs::create_dir_all(root.join("backups")).unwrap();
        std::fs::write(root.join("backups/orquestra_backup_1.tar.gz"), "old").unwrap();

        let archive = create_backup_at(&root).expect("backup succeeds");
        let names = entry_names(&archive);
        assert!(names.iter().any(|name| name == "vault.json"), "{names:?}");
        assert!(
            names.iter().any(|name| name.starts_with("active/")),
            "{names:?}"
        );
        assert!(
            !names.iter().any(|name| name.contains("backups")),
            "a backup must not contain itself: {names:?}"
        );

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
