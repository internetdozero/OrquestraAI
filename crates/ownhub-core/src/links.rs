//! Directory links, per platform.
//!
//! A projection is one directory that resolves to another: Unix uses a
//! symlink, Windows uses an NTFS junction because creating a symlink there
//! needs `SeCreateSymbolicLinkPrivilege` (or Developer Mode) while a junction
//! needs nothing. Both are reparse points, so the *detection* helpers never
//! follow the link and never confuse it with the vault folder behind it.
//!
//! Comparisons normalize the `\\?\` verbatim prefix that `std::fs::canonicalize`
//! returns on Windows; without that, `\\?\C:\...\skills-library\alpha` and
//! `C:\...\skills-library\alpha` compare unequal and every link looks broken.

use std::io;
use std::path::{Path, PathBuf};

/// Create the directory link `link` pointing at `target`.
///
/// The target must already exist: a junction to a missing folder is broken
/// on creation, while a Unix symlink is allowed to dangle. Callers that care
/// resolve the target first, exactly as the projection code does.
pub fn create_dir_link(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        // `junction::create` writes `link`; the crate strips nothing on its
        // own, so a canonicalized target keeps its `\\?\` prefix here.
        junction::create(target, link)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(unsupported())
    }
}

/// True when `path` is a directory link, checked without following it.
///
/// On Windows any reparse point counts: junctions, directory symlinks and
/// cloud placeholders all make `remove_dir_all` follow or delete the target,
/// so the fail-closed answer is "treat it as a link".
pub fn is_dir_link(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT: set for junctions and symlinks alike.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// Remove the link itself, leaving its target and contents untouched.
pub fn remove_dir_link(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        // A junction is a directory entry: `remove_file` fails with
        // "access is denied", while `remove_dir` drops the reparse point and
        // leaves the target alone.
        if std::fs::remove_dir(path).is_err() {
            return std::fs::remove_file(path);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_file(path)
    }
}

/// The target recorded inside the link, without resolving it.
pub fn read_dir_link(path: &Path) -> io::Result<PathBuf> {
    #[cfg(windows)]
    {
        junction::get_target(path)
    }
    #[cfg(not(windows))]
    {
        std::fs::read_link(path)
    }
}

/// Drop the Windows verbatim (`\\?\`) prefix.
///
/// `std::fs::canonicalize` returns `\\?\C:\Users\…` (and
/// `\\?\UNC\server\share\…` for network paths). The same folder spelled
/// without the prefix must compare equal, so both sides of every comparison
/// go through here.
pub fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path.to_path_buf()
}

/// [`strip_verbatim`] where it matters: Windows only.
pub fn normalize(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        strip_verbatim(path)
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

/// Create the file link `link` pointing at `target`.
///
/// A junction only describes a directory, so the instruction files the bridge
/// links (`AGENTS.md` and friends) need their own primitive. On Windows a file
/// *symlink* needs `SeCreateSymbolicLinkPrivilege` (or Developer Mode), which a
/// plain user profile does not have, so the link is a **hard link** instead:
/// no privilege, and the same bytes — editing one name edits the other. Where a
/// hard link is impossible (another volume), the file is copied.
pub fn create_file_link(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        match std::fs::hard_link(target, link) {
            Ok(()) => Ok(()),
            // `ERROR_NOT_SAME_DEVICE`: the projection lives on another volume.
            Err(_) => std::fs::copy(target, link).map(|_| ()),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(unsupported())
    }
}

/// True when `link` already carries `target`'s bytes: a symlink that resolves
/// there, or the Windows hard link [`create_file_link`] makes (whose
/// `canonicalize` answers its own name, not the target's).
pub fn file_matches(link: &Path, target: &Path) -> bool {
    if resolves_to(link, target) {
        return true;
    }
    let (Ok(link_meta), Ok(target_meta)) = (std::fs::metadata(link), std::fs::metadata(target))
    else {
        return false;
    };
    link_meta.is_file()
        && target_meta.is_file()
        && link_meta.len() == target_meta.len()
        && std::fs::read(link).ok() == std::fs::read(target).ok()
}

/// True when `link` is a directory link that resolves to `target`.
///
/// Both sides are canonicalized and normalized, so a junction and a symlink
/// answer the same way and a Windows verbatim prefix cannot make an intact
/// projection look stale (which would replace it, or leave it behind when
/// the operator disables a skill).
pub fn resolves_to(link: &Path, target: &Path) -> bool {
    if !is_dir_link(link) {
        return false;
    }
    match (std::fs::canonicalize(link), std::fs::canonicalize(target)) {
        (Ok(current), Ok(expected)) => normalize(&current) == normalize(&expected),
        _ => false,
    }
}

#[cfg(not(any(unix, windows)))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "directory links are not supported on this platform",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_drive_prefixes_are_dropped() {
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\C:\Users\system\.ownhub\active\codex")),
            PathBuf::from(r"C:\Users\system\.ownhub\active\codex")
        );
    }

    #[test]
    fn verbatim_unc_prefixes_become_unc_paths() {
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\UNC\server\share\skills")),
            PathBuf::from(r"\\server\share\skills")
        );
    }

    #[test]
    fn plain_paths_pass_through_unchanged() {
        assert_eq!(
            strip_verbatim(Path::new("/home/system/.ownhub/active/codex")),
            PathBuf::from("/home/system/.ownhub/active/codex")
        );
        assert_eq!(
            strip_verbatim(Path::new(r"C:\Users\system\.ownhub")),
            PathBuf::from(r"C:\Users\system\.ownhub")
        );
    }

    #[test]
    fn a_hard_linked_file_matches_until_the_master_is_replaced() {
        let root = std::env::temp_dir().join(format!(
            "ownhub-hardlink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).expect("root");
        let master = root.join("AGENTS.md");
        let link = root.join("codex-AGENTS.md");
        std::fs::write(&master, "shared rules\n").expect("master");

        std::fs::hard_link(&master, &link).expect("hard link");
        assert!(file_matches(&link, &master));

        // `write_atomic` publishes through a rename: the master gets a new
        // inode and the old name keeps the previous text, which is why fusion
        // re-points file links after importing instructions.
        let temporary = root.join("AGENTS.md.tmp");
        std::fs::write(&temporary, "shared rules\nmore\n").expect("rewrite");
        std::fs::rename(&temporary, &master).expect("publish");
        assert!(
            !file_matches(&link, &master),
            "a rename must leave the old link visibly stale"
        );
        assert_eq!(
            std::fs::read_to_string(&link).expect("old text"),
            "shared rules\n"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_resolves_and_losing_the_target_breaks_it() {
        let root = std::env::temp_dir().join(format!(
            "ownhub-links-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let vault = root.join("vault/skill");
        let link = root.join("active/codex/skill");
        std::fs::create_dir_all(&vault).expect("vault skill");
        std::fs::create_dir_all(link.parent().expect("parent")).expect("active folder");

        create_dir_link(&vault, &link).expect("link");
        assert!(is_dir_link(&link));
        assert!(resolves_to(&link, &vault));
        assert_eq!(read_dir_link(&link).expect("target"), vault);

        remove_dir_link(&link).expect("unlink");
        assert!(!is_dir_link(&link));
        assert!(
            vault.is_dir(),
            "removing the link must keep the vault folder"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
