//! Static security inspection for skill folders.
//!
//! Scans a skill directory for executable/helper scripts so the Store UI can
//! show a clear badge before install. A skill is flagged when it holds:
//! - files ending in `.sh`, `.bash`, `.py`, `.js` or `.mjs`, or
//! - any file inside a `scripts/` or `bin/` subfolder.
//!
//! The walk is bounded (depth + file cap) and skips `.git`, `target` and
//! `node_modules` so a large checkout cannot stall the import path.

use std::fs;
use std::path::Path;

/// Script extensions that mark a skill as executable.
const SCRIPT_EXTENSIONS: &[&str] = &["sh", "bash", "py", "js", "mjs"];

/// Folder names whose contents always count as executable helpers.
const SCRIPT_DIRS: &[&str] = &["scripts", "bin"];

const MAX_FILES: usize = 500;
const MAX_DEPTH: usize = 4;

/// Scan `dir` and return `(has_scripts, script_files)` with repository-
/// relative, forward-slash paths sorted for stable snapshots.
pub(crate) fn scan_skill_security(dir: &Path) -> (bool, Vec<String>) {
    let mut found: Vec<String> = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];
    let mut visited = 0usize;

    while let Some((current, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            continue;
        }
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_FILES {
                break;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" || name == "target" || name == "node_modules" {
                continue;
            }
            // `DirEntry::file_type` does not follow symlinks. Following a
            // directory link would spend the file cap outside the skill and
            // hide a real `scripts/` payload.
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(_) => continue,
            };
            if kind.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            let relative = path
                .strip_prefix(dir)
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| name.clone());
            // Only ancestor components count, not the file itself.
            let parts: Vec<&str> = relative.split('/').collect();
            let in_helper_dir = parts
                .iter()
                .take(parts.len().saturating_sub(1))
                .any(|part| SCRIPT_DIRS.contains(&part.to_lowercase().as_str()));
            let extension = path
                .extension()
                .map(|ext| ext.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let named_helper = SCRIPT_DIRS.contains(&name.to_lowercase().as_str());
            let looks_like_script =
                in_helper_dir || SCRIPT_EXTENSIONS.contains(&extension.as_str());
            if kind.is_symlink() {
                if looks_like_script || named_helper {
                    found.push(relative);
                }
                continue;
            }
            if kind.is_file() && looks_like_script {
                found.push(relative);
            }
        }
    }

    found.sort();
    found.dedup();
    let has_scripts = !found.is_empty();
    (has_scripts, found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::path::PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-security-{label}-{}-{unique}",
            std::process::id()
        ));
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }
        fs::create_dir_all(&root).expect("scratch dir");
        root
    }

    #[test]
    fn clean_prompt_only_skill_has_no_scripts() {
        let root = scratch("clean");
        fs::write(root.join("SKILL.md"), "# Clean\n\nPrompt only.\n").expect("write skill");
        let (has_scripts, files) = scan_skill_security(&root);
        assert!(!has_scripts);
        assert!(files.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_script_extensions() {
        let root = scratch("ext");
        fs::write(root.join("SKILL.md"), "# Demo\n").expect("write skill");
        fs::write(root.join("helper.py"), "print('hi')\n").expect("write py");
        fs::write(root.join("run.sh"), "#!/bin/sh\n").expect("write sh");
        fs::write(root.join("notes.md"), "# notes\n").expect("write md");
        let (has_scripts, files) = scan_skill_security(&root);
        assert!(has_scripts);
        assert!(files.contains(&"helper.py".to_string()));
        assert!(files.contains(&"run.sh".to_string()));
        assert!(!files.contains(&"notes.md".to_string()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_helper_dirs_and_nested_levels() {
        let root = scratch("dirs");
        fs::create_dir_all(root.join("scripts")).expect("scripts dir");
        fs::create_dir_all(root.join("bin")).expect("bin dir");
        fs::write(root.join("SKILL.md"), "# Demo\n").expect("write skill");
        fs::write(root.join("scripts/deploy"), "#!/bin/sh\n").expect("write helper");
        fs::write(root.join("bin/tool"), "#!/bin/sh\n").expect("write bin");
        let (has_scripts, files) = scan_skill_security(&root);
        assert!(has_scripts);
        assert!(files.iter().any(|file| file.starts_with("scripts/")));
        assert!(files.iter().any(|file| file.starts_with("bin/")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn symlink_directories_are_not_followed() {
        let root = scratch("link");
        let outside = scratch("outside");
        fs::create_dir_all(outside.join("scripts")).expect("outside scripts");
        fs::write(outside.join("scripts/steal.sh"), "#!/bin/sh\n").expect("outside script");
        std::os::unix::fs::symlink(outside.join("scripts"), root.join("scripts")).expect("link");
        fs::write(root.join("SKILL.md"), "# Demo\n").expect("skill");
        fs::write(root.join("real.sh"), "#!/bin/sh\n").expect("real script");
        std::os::unix::fs::symlink("/bin/true", root.join("hook.py")).expect("file link");

        let (has_scripts, files) = scan_skill_security(&root);
        assert!(has_scripts);
        assert!(files.contains(&"real.sh".to_string()));
        assert!(files.contains(&"hook.py".to_string()));
        assert!(
            files.contains(&"scripts".to_string()),
            "a scripts symlink is itself a helper"
        );
        assert!(
            files.iter().all(|file| !file.contains("steal")),
            "the walk must not enter the symlink target: {files:?}"
        );
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }
}
