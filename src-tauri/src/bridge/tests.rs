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

    #[test]
    fn fusion_neutralises_secondary_discovery_paths() {
        let test = TestHome::new("secondary");

        // Primary roots so every harness counts as installed and supported.
        test.skill(".gemini/config/skills", "alpha");
        test.mkdir(".codex/skills");
        test.mkdir(".claude/skills");
        test.mkdir(".hermes/skills");
        test.mkdir(".grok/skills");
        test.mkdir(".copilot/skills");
        test.mkdir(".freebuff/skills");
        test.mkdir(".config/manicode");
        test.mkdir(".config/kilo");
        test.mkdir(".config/opencode");

        // Codex: curated vendor checkout, two caches, and a skill buried five
        // folders deep inside a bundled marketplace plugin.
        test.skill(".codex/vendor_imports/skills/skills/.curated", "curated-one");
        test.write(".codex/vendor_imports/skills/.gitignore", "/.tmp\n");
        test.skill(
            ".codex/.tmp/bundled-marketplaces/openai-bundled/plugins/latex/skills",
            "latex-compile",
        );
        test.skill(".codex/.tmp/marketplaces/ponytail/skills", "ponytail-audit");

        // The shared root Codex and Grok both inject into their sessions.
        test.skill(".agents/skills", "orca-cli");
        test.write(".agents/.skill-lock.json", "{\n  \"version\": 3\n}\n");

        // Grok: bundled built-ins plus an installed plugin bundle.
        test.skill(".grok/bundled/skills", "docx");
        test.skill(".grok/installed-plugins/ponytail/skills", "ponytail-debt");

        // Claude: an official marketplace plugin.
        test.skill(
            ".claude/plugins/marketplaces/claude-plugins-official/plugins/feature-dev/skills",
            "feature-dev",
        );

        // Hermes: the optional repository nests category folders.
        test.skill(".hermes/hermes-agent/optional-skills/security", "threat-model");

        // Antigravity: vendor built-ins are archived but stay installable.
        test.skill(".gemini/antigravity/builtin/skills", "antigravity_guide");

        // The vault already owns the one skill Antigravity exposes, so the
        // adopted list below only covers secondary locations.
        let seed = test.paths.vault.join("alpha");
        fs::create_dir_all(&seed).unwrap();
        fs::write(seed.join("SKILL.md"), "seed\n").unwrap();

        let report = deploy_at(&test.home, &test.paths).unwrap();
        let backup = PathBuf::from(report.backup_dir.clone().unwrap());

        // Every unmanaged skill is archived in the master vault before its
        // source folder is emptied or replaced.
        let mut adopted = report.adopted.clone();
        adopted.sort();
        assert_eq!(
            adopted,
            vec![
                "antigravity_guide",
                "curated-one",
                "docx",
                "feature-dev",
                "latex-compile",
                "orca-cli",
                "ponytail-audit",
                "ponytail-debt",
                "threat-model",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        for name in &adopted {
            assert!(
                test.paths.vault.join(name).join("SKILL.md").is_file(),
                "{name} was not archived in the vault"
            );
        }

        // Codex caches are emptied, never deleted: the originals sit in the run
        // backup and Codex resolves skills only through ~/.codex/skills.
        for rel in [
            ".codex/vendor_imports/skills",
            ".codex/.tmp/bundled-marketplaces",
            ".codex/.tmp/marketplaces",
        ] {
            let path = test.home.join(rel);
            assert!(path.is_dir(), "{rel} should be left in place");
            assert!(
                fs::read_dir(&path).unwrap().next().is_none(),
                "{rel} still feeds Codex skills"
            );
        }
        assert!(backup
            .join("codex/.codex__vendor_imports__skills/skills/.curated/curated-one/SKILL.md")
            .is_file());
        assert!(backup
            .join("codex/.codex__vendor_imports__skills/.gitignore")
            .is_file());
        assert!(backup
            .join("codex/.codex__.tmp__bundled-marketplaces/openai-bundled/plugins/latex/skills/latex-compile/SKILL.md")
            .is_file());
        assert!(backup
            .join("codex/.codex__.tmp__marketplaces/ponytail/skills/ponytail-audit/SKILL.md")
            .is_file());

        // Claude and Hermes stop discovering marketplace skills, and the shared
        // root both Codex and Grok read is emptied too.
        for rel in [
            ".claude/plugins/marketplaces",
            ".hermes/hermes-agent/optional-skills",
            ".grok/installed-plugins",
            ".agents/skills",
        ] {
            let path = test.home.join(rel);
            assert!(path.is_dir(), "{rel} should be left in place");
            assert!(
                fs::read_dir(&path).unwrap().next().is_none(),
                "{rel} still feeds unmanaged skills"
            );
        }
        assert!(backup
            .join("claude/.claude__plugins__marketplaces/claude-plugins-official/plugins/feature-dev/skills/feature-dev/SKILL.md")
            .is_file());
        assert!(backup
            .join("hermes/.hermes__hermes-agent__optional-skills/security/threat-model/SKILL.md")
            .is_file());
        assert!(backup
            .join("grok/.grok__installed-plugins/ponytail/skills/ponytail-debt/SKILL.md")
            .is_file());
        assert!(backup
            .join("codex/.agents__skills/orca-cli/SKILL.md")
            .is_file());
        // The shared root's own bookkeeping stays where the CLI expects it.
        assert!(test.home.join(".agents/.skill-lock.json").is_file());

        // Grok resolves its built-ins through the projection instead of a
        // folder it reads directly.
        assert_linked(
            &test.home.join(".grok/bundled/skills"),
            &test.paths.projection_dir("grok"),
        );
        assert!(backup
            .join("grok/.grok__bundled__skills/docx/SKILL.md")
            .is_file());

        // Antigravity keeps its vendor built-ins usable; they are only
        // archived so ~/.gemini/config/skills stays the single user source.
        assert!(test
            .home
            .join(".gemini/antigravity/builtin/skills/antigravity_guide/SKILL.md")
            .is_file());
        assert_linked(
            &test.home.join(".gemini/config/skills"),
            &test.paths.projection_dir("antigravity"),
        );

        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        assert!(report
            .quarantined
            .contains(&".codex/vendor_imports/skills".to_string()));
        assert!(report.quarantined.contains(&".agents/skills".to_string()));
        assert!(report.quarantined.contains(&".grok/bundled/skills".to_string()));
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
                    status.secondary_cleared,
                    status
                        .secondary_paths
                        .iter()
                        .map(|entry| (entry.path.clone(), entry.cleared))
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>()
        );

        // Idempotent: a second run re-archives and re-quarantines nothing.
        let second = deploy_at(&test.home, &test.paths).unwrap();
        assert!(second.backup_dir.is_none());
        assert!(second.quarantined.is_empty());
        assert!(second.adopted.is_empty());
        assert_eq!(second.bridged, second.total);
        assert!(second.errors.is_empty());

        test.cleanup();
    }

    #[test]
    fn config_merge_refuses_to_corrupt_invalid_jsonc() {
        let content = "{ this is not: valid ]\n";
        assert!(merge_skills_path(content, "/x").is_err());
    }
}
