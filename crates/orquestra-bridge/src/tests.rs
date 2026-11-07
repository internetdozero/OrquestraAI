#![allow(clippy::unwrap_used)]

//! Unit tests for fusion: backups, link state, config merging and the
//! secondary discovery cleanup.

use super::*;
use std::fs;
use std::path::{Path, PathBuf};

use orquestra_core::paths::{rebase, OrquestraPaths};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestHome {
        home: PathBuf,
        paths: OrquestraPaths,
    }

    impl TestHome {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let base = std::env::temp_dir().join(format!(
                "orquestra-bridge-{label}-{}-{unique}",
                std::process::id()
            ));
            let home = base.join("home");
            let root = base.join("orquestra");

            fs::create_dir_all(&home).unwrap();

            TestHome {
                home,
                paths: OrquestraPaths {
                    vault: root.join("skills-library"),
                    active: root.join("active"),
                    root,
                },
            }
        }

        /// Fixture path for a registry-style relative path, resolved exactly
        /// like production (`paths::rebase`), so the suite tests the platform
        /// layout instead of the Linux literal on every runner.
        fn path(&self, rel: &str) -> PathBuf {
            rebase(&self.home, rel)
        }

        fn write(&self, rel: &str, content: &str) -> PathBuf {
            let path = self.path(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
            path
        }

        fn mkdir(&self, rel: &str) -> PathBuf {
            let path = self.path(rel);
            fs::create_dir_all(&path).unwrap();
            path
        }

        fn skill(&self, rel: &str, name: &str) -> PathBuf {
            let directory = self.path(rel).join(name);
            fs::create_dir_all(&directory).unwrap();
            fs::write(
                directory.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: {name} skill.\n---\n\n{name} body.\n"),
            )
            .unwrap();
            directory
        }

        fn cleanup(&self) {
            let _ = fs::remove_dir_all(self.home.parent().unwrap());
        }
    }

    fn assert_linked(link: &Path, expected_owner: &Path) {
        assert!(is_link(link), "{} is not a symlink", link.display());
        assert_eq!(
            fs::canonicalize(link).unwrap(),
            fs::canonicalize(expected_owner).unwrap(),
            "{} points at the wrong projection",
            link.display()
        );
    }

    #[test]
    fn formats_utc_timestamps() {
        assert_eq!(format_timestamp(0), "19700101-000000");
        assert_eq!(format_timestamp(1_700_000_000), "20231114-221320");
        assert_eq!(format_timestamp(1_772_000_000), "20260225-061320");
    }

    #[test]
    fn merges_skills_path_into_jsonc_without_losing_keys() {
        let content = "{\n  // keep this comment\n  \"permission\": { \"bash\": \"ask\" },\n}\n";
        let scope = "/home/system/.orquestra/active/kilo";

        let updated = merge_skills_path(content, scope).unwrap().unwrap();
        assert!(updated.contains("// keep this comment"));

        let value: serde_json::Value = json5::from_str(&updated).unwrap();
        assert_eq!(
            value
                .get("skills")
                .and_then(|skills| skills.get("paths"))
                .and_then(|paths| paths.as_array())
                .and_then(|paths| paths.first())
                .and_then(|entry| entry.as_str()),
            Some(scope)
        );
        assert_eq!(
            value
                .get("permission")
                .and_then(|permission| permission.get("bash")),
            Some(&serde_json::Value::String("ask".to_string()))
        );

        assert!(merge_skills_path(&updated, scope).unwrap().is_none());
    }

    #[test]
    fn merges_into_existing_skills_block() {
        let content =
            "{\n  \"skills\": { \"paths\": [\"/other\"] },\n  \"mcp\": { \"rea\": true }\n}\n";
        let scope = "/home/system/.orquestra/active/opencode";

        let updated = merge_skills_path(content, scope).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_str(&updated).unwrap();
        let paths = value
            .get("skills")
            .and_then(|skills| skills.get("paths"))
            .and_then(|paths| paths.as_array())
            .unwrap();

        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].as_str(), Some("/other"));
        assert_eq!(paths[1].as_str(), Some(scope));
        assert!(value.get("mcp").is_some());
    }

    /// A marketplace folder can point a skill name at a tree outside the
    /// harness. `read_dir` follows that link, so fusion would copy the target
    /// into the vault. The walk must only see real directories.
    #[cfg(unix)]
    #[test]
    fn skill_walk_does_not_follow_a_directory_symlink() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = orquestra_core::test_support::root().join(format!(
            "orquestra-bridge-skill-link-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let harness = root.join("skills");
        let outside = root.join("outside");
        let nested_out = root.join("nested-out");
        fs::create_dir_all(harness.join("kept")).unwrap();
        fs::write(harness.join("kept/SKILL.md"), "# Kept\n").unwrap();
        fs::create_dir_all(outside.join("scripts")).unwrap();
        fs::write(outside.join("SKILL.md"), "# Outside\n").unwrap();
        fs::write(outside.join("scripts/steal.sh"), "ssh-private\n").unwrap();
        fs::create_dir_all(nested_out.join("hidden")).unwrap();
        fs::write(nested_out.join("hidden/SKILL.md"), "# Hidden\n").unwrap();
        std::os::unix::fs::symlink(&outside, harness.join("linked")).unwrap();
        std::os::unix::fs::symlink(&nested_out, harness.join("via")).unwrap();

        let found = skill_folders(&harness, PRIMARY_WALK).expect("walk");
        let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["kept"]);
        assert!(
            found
                .iter()
                .all(|(_, path)| { !path.starts_with(&outside) && !path.starts_with(&nested_out) }),
            "walk returned a path outside the harness: {found:?}"
        );
        assert!(fs::read_to_string(outside.join("scripts/steal.sh"))
            .unwrap()
            .contains("ssh-private"));

        let _ = fs::remove_dir_all(&root);
    }

    mod fusion;
    mod instructions;
    mod secondary;
    mod shared_root;
}
