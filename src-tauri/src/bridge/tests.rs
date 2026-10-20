#![allow(clippy::unwrap_used)]

//! Unit tests for fusion: backups, link state, config merging and the
//! secondary discovery cleanup.

use super::*;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::OrquestraPaths;

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

        fn write(&self, rel: &str, content: &str) -> PathBuf {
            let path = self.home.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
            path
        }

        fn mkdir(&self, rel: &str) -> PathBuf {
            let path = self.home.join(rel);
            fs::create_dir_all(&path).unwrap();
            path
        }

        fn skill(&self, rel: &str, name: &str) -> PathBuf {
            let directory = self.home.join(rel).join(name);
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
            value.get("permission").and_then(|permission| permission.get("bash")),
            Some(&serde_json::Value::String("ask".to_string()))
        );

        assert!(merge_skills_path(&updated, scope).unwrap().is_none());
    }

    #[test]
    fn merges_into_existing_skills_block() {
        let content = "{\n  \"skills\": { \"paths\": [\"/other\"] },\n  \"mcp\": { \"rea\": true }\n}\n";
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

    #[test]
    fn fusion_backs_up_links_and_preserves_harness_state() {
        let test = TestHome::new("fusion");

        // Antigravity: real skills dir plus an instructions file.
        test.skill(".gemini/config/skills", "alpha");
        test.write(".gemini/config/AGENTS.md", "Antigravity house rules\n");

        // Codex: numbered skills plus the internal .system folder.
        test.skill(".codex/skills", "beta");
        test.write(".codex/skills/.system/imagegen/SKILL.md", "system skill\n");
        test.write(".codex/AGENTS.md", "Codex house rules\n");

        // Claude: empty instruction file.
        test.skill(".claude/skills", "gamma");
        test.write(".claude/CLAUDE.md", "");

        // Hermes: nested categories plus curator state.
        test.skill(".hermes/skills", "delta");
        test.write(
            ".hermes/skills/security/nested/SKILL.md",
            "---\nname: nested\ndescription: Nested skill.\n---\n\nnested body.\n",
        );
        test.write(".hermes/skills/.curator_state", "state\n");

        // Grok: a skill that is missing from the vault and must be adopted.
        test.skill(".grok/skills", "epsilon");

        // Copilot, Freebuff and Manicode.
        test.skill(".copilot/skills", "zeta");
        test.mkdir(".freebuff/worktrees");
        test.mkdir(".config/manicode");

        // Kilo has a JSONC config; OpenCode has a directory but no config yet.
        test.write(
            ".config/kilo/kilo.jsonc",
            "{\n  // keep me\n  \"permission\": { \"bash\": \"ask\" },\n}\n",
        );
        test.mkdir(".config/opencode");

        // Vault already holds every skill except epsilon.
        for name in ["alpha", "beta", "gamma", "delta", "zeta"] {
            let directory = test.paths.vault.join(name);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("SKILL.md"), "seed\n").unwrap();
        }

        let report = deploy_at(&test.home, &test.paths).unwrap();

        let backup = PathBuf::from(report.backup_dir.clone().unwrap());
        assert!(backup.file_name().unwrap().to_string_lossy().starts_with("pre-fusion-"));

        // Originals are preserved in the backup.
        assert!(backup.join("antigravity/.gemini__config__skills/alpha/SKILL.md").is_file());
        assert!(backup.join("codex/.codex__skills/beta/SKILL.md").is_file());
        assert_eq!(
            fs::read_to_string(backup.join("antigravity/.gemini__config__AGENTS.md")).unwrap(),
            "Antigravity house rules\n"
        );

        // Skills directories now point at their projection.
        assert_linked(
            &test.home.join(".gemini/config/skills"),
            &test.paths.projection_dir("antigravity"),
        );
        assert_linked(
            &test.home.join(".codex/skills"),
            &test.paths.projection_dir("codex"),
        );
        assert_linked(
            &test.home.join(".claude/skills"),
            &test.paths.projection_dir("claude"),
        );
        assert_linked(
            &test.home.join(".hermes/skills"),
            &test.paths.projection_dir("hermes"),
        );
        assert_linked(
            &test.home.join(".grok/skills"),
            &test.paths.projection_dir("grok"),
        );
        assert_linked(
            &test.home.join(".copilot/skills"),
            &test.paths.projection_dir("copilot"),
        );
        assert_linked(
            &test.home.join(".freebuff/skills"),
            &test.paths.projection_dir("freebuff"),
        );
        assert_linked(
            &test.home.join(".config/manicode/skills"),
            &test.paths.projection_dir("freebuff"),
        );

        // Harness-owned state survives inside the projection.
        assert!(test.paths.projection_dir("codex").join(".system/imagegen/SKILL.md").is_file());
        assert!(test.paths.projection_dir("hermes").join(".curator_state").is_file());

        // Fusion merges rather than resets: each harness keeps the skills it
        // already had visible, now linked from the vault.
        for (scope, skill) in [
            ("antigravity", "alpha"),
            ("codex", "beta"),
            ("claude", "gamma"),
            ("hermes", "delta"),
            ("grok", "epsilon"),
            ("copilot", "zeta"),
        ] {
            assert_linked(
                &test.paths.projection_dir(scope).join(skill),
                &test.paths.vault.join(skill),
            );
        }

        // Instructions now resolve to the master file.
        assert_linked(
            &test.home.join(".gemini/config/AGENTS.md"),
            &test.paths.agents_path(),
        );
        assert_linked(&test.home.join(".codex/AGENTS.md"), &test.paths.agents_path());
        assert_linked(
            &test.home.join(".config/manicode/AGENTS.md"),
            &test.paths.agents_path(),
        );

        // Harness-only skills (flat and nested) were adopted instead of
        // disappearing when the folders were replaced.
        let mut adopted = report.adopted.clone();
        adopted.sort();
        assert_eq!(adopted, vec!["epsilon".to_string(), "nested".to_string()]);
        assert!(test.paths.vault.join("epsilon/SKILL.md").is_file());
        assert!(test.paths.vault.join("nested/SKILL.md").is_file());
        assert_linked(
            &test.paths.projection_dir("hermes").join("nested"),
            &test.paths.vault.join("nested"),
        );

        // Kilo config keeps its comment and gains the projection path.
        let kilo = fs::read_to_string(test.home.join(".config/kilo/kilo.jsonc")).unwrap();
        assert!(kilo.contains("// keep me"));
        assert!(kilo.contains(&test.paths.projection_dir("kilo").to_string_lossy().into_owned()));

        // OpenCode config is created for an installed harness.
        let opencode = fs::read_to_string(test.home.join(".config/opencode/opencode.jsonc")).unwrap();
        assert!(opencode.contains(&test.paths.projection_dir("opencode").to_string_lossy().into_owned()));

        // Harness instructions were folded into the pristine master file.
        let master = fs::read_to_string(test.paths.agents_path()).unwrap();
        assert!(master.contains("## Imported from Antigravity"));
        assert!(master.contains("Antigravity house rules"));
        assert!(master.contains("## Imported from Codex"));
        assert!(master.contains("Codex house rules"));

        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        assert_eq!(
            report.bridged,
            report.total,
            "unbridged: {:?}",
            report
                .harnesses
                .iter()
                .filter(|status| !status.bridged)
                .map(|status| (
                    status.id.clone(),
                    status.installed,
                    status.skills_linked,
                    status.instructions_linked,
                    status.config_linked
                ))
                .collect::<Vec<_>>()
        );

        // A second run is a no-op with no new backup.
        let second = deploy_at(&test.home, &test.paths).unwrap();
        assert!(second.backup_dir.is_none());
        assert_eq!(second.bridged, second.total);
        assert!(second.errors.is_empty());

        test.cleanup();
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
        let root = PathBuf::from("/var/tmp/testing").join(format!(
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
            found.iter().all(|(_, path)| {
                !path.starts_with(&outside) && !path.starts_with(&nested_out)
            }),
            "walk returned a path outside the harness: {found:?}"
        );
        assert!(
            fs::read_to_string(outside.join("scripts/steal.sh"))
                .unwrap()
                .contains("ssh-private")
        );

        let _ = fs::remove_dir_all(&root);
    }

    mod secondary;

}
