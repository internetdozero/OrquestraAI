//! Worktree directory resolver for desktop threads.
//!
//! Desktop threads are 36-char UUIDs but their worktree folders can use the
//! exact UUID or an 8-char hash prefix. This resolver correlates the two so
//! sizes are counted once and deletes reclaim the right folder.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Locate the worktree folder for `thread_id` inside `worktrees_dir`.
///
/// Exact match wins; otherwise an 8-char prefix match is used. Traversal
/// segments are refused outright.
pub fn find_worktree(worktrees_dir: &Path, thread_id: &str) -> Option<PathBuf> {
    if thread_id.is_empty()
        || thread_id == "."
        || thread_id == ".."
        || thread_id.contains('/')
        || thread_id.contains('\\')
        || thread_id.contains('\0')
    {
        return None;
    }

    // `is_dir` and `read_dir` follow. A symlinked worktrees folder would
    // resolve, and the desktop delete would remove that outside tree.
    let real_dir = std::fs::symlink_metadata(worktrees_dir)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false);
    if !real_dir {
        return None;
    }

    let exact = worktrees_dir.join(thread_id);
    if exact.is_dir() {
        return Some(exact);
    }

    let prefix = thread_id.get(..8)?;
    if thread_id.len() < 8 {
        return None;
    }

    let Ok(entries) = std::fs::read_dir(worktrees_dir) else {
        return None;
    };

    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|k| k.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        // 8-char short-hash folder, or a longer folder name the thread starts with.
        if name == prefix || (name.len() >= 8 && thread_id.starts_with(&name)) {
            candidates.push((name, entry.path()));
        }
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.into_iter().next().map(|(_, path)| path)
}

/// Directory names under `worktrees_dir` claimed by the given thread ids.
pub fn claimed_dir_names(worktrees_dir: &Path, thread_ids: &[String]) -> HashSet<String> {
    let mut claimed = HashSet::new();
    for thread in thread_ids {
        if let Some(path) = find_worktree(worktrees_dir, thread) {
            if let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) {
                claimed.insert(name);
            }
        }
    }
    claimed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(label: &str) -> PathBuf {
        let root = PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-worktree-{}-{}",
            label,
            std::process::id()
        ));
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }
        fs::create_dir_all(&root).expect("scratch");
        root
    }

    #[test]
    fn exact_match_wins_over_prefix() {
        let root = scratch("exact");
        let thread = "cb07e0aa-1111-2222-3333-444444444444";
        fs::create_dir_all(root.join(thread)).unwrap();
        fs::create_dir_all(root.join("cb07e0aa")).unwrap();
        assert_eq!(find_worktree(&root, thread), Some(root.join(thread)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prefix_match_finds_short_hash() {
        let root = scratch("prefix");
        fs::create_dir_all(root.join("cb07e0aa")).unwrap();
        let thread = "cb07e0aa-1111-2222-3333-444444444444";
        assert_eq!(find_worktree(&root, thread), Some(root.join("cb07e0aa")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_traversal_and_short_ids() {
        let root = scratch("reject");
        fs::create_dir_all(&root).unwrap();
        assert!(find_worktree(&root, "../escape").is_none());
        assert!(find_worktree(&root, "a/b").is_none());
        assert!(find_worktree(&root, "").is_none());
        assert!(find_worktree(&root, "short").is_none());
        let _ = fs::remove_dir_all(&root);
    }
}
