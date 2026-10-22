//! Byte counts and mtimes that stay inside the store they are asked about.
//!
//! Sizing never follows a symlink and prunes dependency/VCS/build trees:
//! a worktree-backed session would otherwise make every listing walk a
//! `node_modules` with hundreds of thousands of files.

use std::fs;
use std::path::Path;

/// Directory names never descended into when sizing. Dependency checkouts,
/// VCS metadata and build caches hold hundreds of thousands of files that
/// the size column should never report; walking them on every listing
/// refresh is what saturates I/O on worktree-backed sessions.
const SKIP_DIRS: [&str; 8] = [
    "node_modules",
    ".git",
    ".hg",
    ".svn",
    "target",
    "__pycache__",
    ".venv",
    "venv",
];

/// Backstop descent budget for [`dir_size`]. Session payloads live within a
/// few levels of the session root, so anything deeper is either a runaway
/// artifact or already covered by [`SKIP_DIRS`].
const DEFAULT_MAX_DEPTH: usize = 8;

fn is_skipped_dir(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|name| SKIP_DIRS.contains(&name))
}

/// Total bytes of every regular file under `path`, without following symlinks.
/// Dependency, VCS and build-cache directories ([`SKIP_DIRS`]) are never
/// descended into, and directories deeper than [`DEFAULT_MAX_DEPTH`] levels
/// below `path` are skipped, so sizing a worktree cannot saturate I/O.
/// Unreadable entries are skipped: a lock file that vanishes mid-walk is not a
/// reason to report the whole harness as broken.
pub fn dir_size(path: &Path) -> u64 {
    dir_size_limited(path, DEFAULT_MAX_DEPTH)
}

/// [`dir_size`] with an explicit descent budget: directories more than
/// `max_depth` levels below `path` are skipped. `0` counts only the files
/// directly inside `path`; the root itself is always walked.
pub fn dir_size_limited(path: &Path, max_depth: usize) -> u64 {
    let mut total = 0;
    let mut stack = vec![(path.to_path_buf(), 0usize)];

    while let Some((current, depth)) = stack.pop() {
        // `read_dir` follows a symlink. A session folder that *is* a link
        // would otherwise be walked, counting (and stalling on) the target.
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            _ => continue,
        }

        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };

            if kind.is_dir() {
                if depth < max_depth && !is_skipped_dir(&entry.file_name()) {
                    stack.push((entry.path(), depth + 1));
                }
            } else if kind.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total += metadata.len();
                }
            }
            // Symlinks and special files are counted as zero on purpose.
        }
    }

    total
}

/// Bytes of a regular file. `None` when the path is missing, a directory, or a
/// symlink: `metadata` and `read_to_string` follow links, so a session file
/// that is a link would be measured and loaded from outside the store. A link
/// also reports length 0, which would skip the size ceiling and read it whole.
pub(in crate::adapters) fn regular_file_len(path: &Path) -> Option<u64> {
    let metadata = fs::symlink_metadata(path).ok()?;
    metadata.file_type().is_file().then_some(metadata.len())
}

/// Size of one regular file, or `0` when it is missing or a symlink.
pub fn file_size(path: &Path) -> u64 {
    regular_file_len(path).unwrap_or(0)
}

/// Modification time in epoch seconds, used only when a store records no
/// timestamp of its own.
pub fn modified_secs(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::{dir_size, dir_size_limited, file_size};
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn a_symlinked_session_tree_is_not_measured() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let scratch = std::path::PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-dir-size-{}-{unique}",
            std::process::id()
        ));
        let outside = scratch.join("outside");
        let session = scratch.join("vault").join("session");
        fs::create_dir_all(&outside).expect("outside tree");
        fs::create_dir_all(session.parent().expect("vault")).expect("vault");
        fs::write(outside.join("secret.bin"), vec![7u8; 4096]).expect("payload");
        fs::write(outside.join("note.txt"), b"note").expect("note");
        std::os::unix::fs::symlink(outside.join("note.txt"), outside.join("alias.txt"))
            .expect("file symlink");
        std::os::unix::fs::symlink(&outside, &session).expect("dir symlink");

        assert_eq!(dir_size(&outside), 4100, "real files still count");
        assert_eq!(dir_size(&session), 0, "the link itself is not walked");
        assert_eq!(
            crate::adapters::fsops::size_of(&session),
            0,
            "reclaim estimates use the same rule"
        );
        assert_eq!(file_size(&outside.join("secret.bin")), 4096);
        assert!(outside.join("secret.bin").is_file(), "the target stays put");

        fs::remove_file(&session).expect("unlink session");
        fs::remove_dir_all(&scratch).expect("cleanup");
    }

    fn unique_scratch(tag: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "orquestra-dir-unit-{tag}-{}-{unique}",
            std::process::id()
        ))
    }

    fn write_sized(path: &std::path::Path, bytes: usize) {
        fs::write(path, vec![7u8; bytes]).expect("fixture file");
    }

    #[test]
    fn heavy_dependency_and_build_dirs_are_pruned() {
        let scratch = unique_scratch("prune");
        let session = scratch.join("session");
        fs::create_dir_all(&session).expect("session dir");
        write_sized(&session.join("thread.json"), 100);

        // Ballast at the top level and nested one level down: none of it
        // may contribute, wherever it sits.
        for heavy in ["node_modules", ".git", "target", "__pycache__", ".venv"] {
            for base in [&session, &session.join("sub")] {
                let dir = base.join(heavy);
                fs::create_dir_all(&dir).expect("ballast dir");
                write_sized(&dir.join("blob.bin"), 4096);
            }
        }

        assert_eq!(dir_size(&session), 100, "only real payload files count");

        fs::remove_dir_all(&scratch).expect("cleanup");
    }

    #[test]
    fn descent_stops_at_the_depth_budget() {
        let scratch = unique_scratch("depth");
        let mut level = scratch.join("root");
        fs::create_dir_all(&level).expect("root");
        // Twelve nested levels, one 10-byte file each.
        for depth in 0..12 {
            write_sized(&level.join(format!("file-{depth}.bin")), 10);
            level = level.join(format!("d{depth}"));
            fs::create_dir_all(&level).expect("nested level");
        }
        let root = scratch.join("root");

        // Budget 2 visits root, d0 and d1: three files.
        assert_eq!(dir_size_limited(&root, 2), 30);
        // Budget 0 counts only the files directly inside the root.
        assert_eq!(dir_size_limited(&root, 0), 10);
        // The default budget of 8 visits root plus eight levels: nine files.
        assert_eq!(dir_size(&root), 90);

        fs::remove_dir_all(&scratch).expect("cleanup");
    }
}
