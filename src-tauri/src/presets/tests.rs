#![allow(clippy::unwrap_used)]

//! Unit tests for the built-in profiles, the preset store and the planning
//! that decides what an apply will project.

use super::apply::{plan_mcps, plan_skills};
use super::scopes::{normalize_scope_map, plan_scope_map, union_unique};
use super::builtins;
use super::store::*;
use super::*;
use std::fs;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::naming::slugify;
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "orquestra-presets-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn builtin(id: &str) -> Preset {
        builtins::all()
            .into_iter()
            .find(|preset| preset.id == id)
            .unwrap_or_else(|| panic!("missing built-in {id}"))
    }

    #[test]
    fn builtins_cover_the_announced_profiles() {
        let all = builtins::all();
        let ids: Vec<&str> = all.iter().map(|preset| preset.id.as_str()).collect();

        assert_eq!(ids, vec!["full-power"]);
        assert!(all.iter().all(|preset| preset.builtin && !preset.active));

        let full = builtin("full-power");
        assert_eq!(full.icon, "zap");
        assert!(full.all_skills && full.all_mcps);
    }

    #[test]
    fn rejects_path_shaped_ids() {
        assert!(validate_id("stealth").is_ok());
        assert!(validate_id("game-dev-2").is_ok());

        for id in ["", ".", "..", ".hidden", "a/b", "a\\b", "two words", "nul\0"] {
            assert!(validate_id(id).is_err(), "{id:?} should be rejected");
        }
    }

    #[test]
    fn slugs_a_name_and_avoids_collisions() {
        assert_eq!(slugify("Game Dev"), "game-dev");
        assert_eq!(slugify("  Bug -- Hunting  "), "bug-hunting");
        assert_eq!(slugify("***"), "preset");

        let file = PresetFile::default();
        assert_eq!(unique_id("Bug Hunting", &file), "bug-hunting");
        assert_eq!(unique_id("Full Power", &file), "full-power-2");

        let mut taken = PresetFile::default();
        taken.custom.push(Preset {
            id: "bug-hunting".to_string(),
            name: "Bug Hunting".to_string(),
            ..builtin("full-power")
        });
        assert_eq!(unique_id("Bug Hunting", &taken), "bug-hunting-2");
    }

    #[test]
    fn plans_known_ids_and_warns_about_the_rest() {
        // A selective profile listing its members explicitly.
        let selective = Preset {
            id: "selective".to_string(),
            name: "Selective".to_string(),
            description: String::new(),
            icon: "cpu".to_string(),
            skills: vec![
                "rtk".to_string(),
                "modular-start".to_string(),
                "github".to_string(),
            ],
            mcps: vec![
                "github".to_string(),
                "rea".to_string(),
                "idea-vault".to_string(),
                "orquestra-core".to_string(),
            ],
            harness_skills: Default::default(),
            harness_mcps: Default::default(),
            all_skills: false,
            all_mcps: false,
            builtin: false,
            active: false,
        };
        let (skills, warnings) = plan_skills(
            &selective,
            &[
                "rtk".to_string(),
                "modular-start".to_string(),
                "github".to_string(),
            ],
        );
        assert_eq!(skills.len(), 3);
        assert!(warnings.is_empty());

        // A vault missing one skill keeps the others and reports the gap.
        let (skills, warnings) = plan_skills(
            &selective,
            &["rtk".to_string(), "github".to_string()],
        );
        assert_eq!(skills.len(), 2);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("modular-start"));

        let (_, warnings) = plan_mcps(&selective, &["github".to_string()]);
        assert_eq!(warnings.len(), 3, "rea, idea-vault and orquestra-core are missing");
    }

    #[test]
    fn full_power_takes_the_whole_known_set() {
        let known: Vec<String> = (0..5).map(|index| format!("skill-{index}")).collect();
        let (skills, warnings) = plan_skills(&builtin("full-power"), &known);
        assert_eq!(skills.len(), 5);
        assert!(warnings.is_empty());

        let (mcps, warnings) = plan_mcps(&builtin("full-power"), &["github".to_string()]);
        assert_eq!(mcps.len(), 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn legacy_presets_without_scope_maps_still_load() {
        // Old presets.json has no harnessSkills/harnessMcps keys: serde
        // defaults them to empty maps and nothing breaks.
        let legacy = serde_json::json!({
            "id": "old",
            "name": "Old",
            "skills": ["rtk"],
            "mcps": ["github"]
        });
        let preset: Preset = serde_json::from_value(legacy).unwrap();
        assert_eq!(preset.skills, vec!["rtk"]);
        assert!(preset.harness_skills.is_empty());
        assert!(preset.harness_mcps.is_empty());
    }

    #[test]
    fn scope_maps_normalize_to_known_harnesses_only() {
        let raw: std::collections::HashMap<String, Vec<String>> = [
            ("antigravity".to_string(), vec!["rtk".to_string(), "rtk".to_string()]),
            ("claude".to_string(), vec!["surgical-patch".to_string()]),
            ("ghost-harness".to_string(), vec!["rtk".to_string()]),
            ("codex".to_string(), Vec::new()),
            ("grok".to_string(), vec!["../escape".to_string()]),
        ]
        .into_iter()
        .collect();

        let cleaned = normalize_scope_map(&raw);
        assert_eq!(cleaned.get("antigravity"), Some(&vec!["rtk".to_string()]));
        assert_eq!(cleaned.get("claude"), Some(&vec!["surgical-patch".to_string()]));
        assert!(!cleaned.contains_key("ghost-harness"));
        assert!(!cleaned.contains_key("codex"), "empty lists are dropped");
        assert!(!cleaned.contains_key("grok"), "path-shaped ids are dropped");
    }

    #[test]
    fn scope_maps_plan_against_installed_ids() {
        let map: std::collections::HashMap<String, Vec<String>> = [
            ("antigravity".to_string(), vec!["rtk".to_string(), "missing".to_string()]),
            ("claude".to_string(), vec!["gone".to_string()]),
        ]
        .into_iter()
        .collect();

        let known = vec!["rtk".to_string()];
        let (planned, warnings) = plan_scope_map(&map, &known, "skill");
        assert_eq!(planned.len(), 1);
        assert!(planned["antigravity"].contains("rtk"));
        assert_eq!(warnings.len(), 2);
        assert!(warnings.iter().any(|w| w.contains("missing")));
        assert!(warnings.iter().any(|w| w.contains("gone")));
    }

    #[test]
    fn report_union_counts_global_and_harness_sets_once() {
        let global: std::collections::BTreeSet<String> =
            ["rtk".to_string()].into_iter().collect();
        let per_harness: std::collections::HashMap<String, std::collections::BTreeSet<String>> = [(
            "antigravity".to_string(),
            ["rtk".to_string(), "github".to_string()].into_iter().collect(),
        )]
        .into_iter()
        .collect();

        let all = union_unique(&global, &per_harness);
        assert_eq!(all.len(), 2, "rtk is counted once across scopes");
    }

    #[test]
    fn store_roundtrips_custom_presets_and_the_active_flag() {
        let root = temp_root("store");

        // A missing file is a normal first run: built-ins, no custom.
        let empty = load_from(&root).unwrap();
        assert!(empty.custom().is_empty());
        assert_eq!(merged(&empty).len(), builtins::all().len());

        let custom = Preset {
            id: "game-dev".to_string(),
            name: "Game Dev".to_string(),
            description: "Engine work.".to_string(),
            icon: "custom".to_string(),
            skills: vec!["rtk".to_string()],
            mcps: vec!["github".to_string()],
            harness_skills: [("antigravity".to_string(), vec!["github".to_string()])]
                .into_iter()
                .collect(),
            harness_mcps: Default::default(),
            all_skills: false,
            all_mcps: false,
            builtin: false,
            active: false,
        };
        let file = PresetFile {
            custom: vec![custom.clone()],
            active: Some("game-dev".to_string()),
            overrides: Vec::new(),
        };
        save_to(&root, &file).unwrap();

        let reloaded = load_from(&root).unwrap();
        assert_eq!(reloaded.custom().len(), 1);
        assert_eq!(reloaded.active(), Some("game-dev"));

        let listed = merged(&reloaded);
        assert_eq!(listed.len(), builtins::all().len() + 1);
        assert!(listed
            .iter()
            .find(|preset| preset.id == "game-dev")
            .unwrap()
            .active);
        assert!(listed
            .iter()
            .find(|preset| preset.id == "full-power")
            .is_some_and(|preset| !preset.active));

        // A stored built-in flag cannot smuggle a deletable built-in.
        let mut forged = reloaded;
        forged.custom[0].builtin = true;
        assert!(!find(&forged, "game-dev").unwrap().builtin);

        // No temp files are left behind.
        let leftovers = fs::read_dir(&root)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        assert_eq!(leftovers, 0);

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn overrides_builtin_and_resets_cleanly() {
        let root = temp_root("overrides");

        // Initially no overrides
        let empty = load_from(&root).unwrap();
        assert!(empty.overrides().is_empty());
        let default_full = builtin("full-power");
        assert_eq!(default_full.name, "Full Power");
        assert!(default_full.all_skills && default_full.all_mcps);

        // Simulate an override written to disk
        let mut overridden_full = default_full.clone();
        overridden_full.name = "Custom Full Power".to_string();
        overridden_full.skills = vec!["rtk".to_string(), "github".to_string()];
        overridden_full.icon = "custom".to_string();

        let file = PresetFile {
            custom: Vec::new(),
            active: Some("full-power".to_string()),
            overrides: vec![overridden_full.clone()],
        };
        save_to(&root, &file).unwrap();

        let reloaded = load_from(&root).unwrap();
        assert_eq!(reloaded.overrides().len(), 1);

        let listed = merged(&reloaded);
        let merged_full = listed.iter().find(|p| p.id == "full-power").unwrap();
        assert_eq!(merged_full.name, "Custom Full Power");
        assert_eq!(merged_full.skills, vec!["rtk", "github"]);
        assert_eq!(merged_full.icon, "custom");
        assert!(merged_full.builtin, "must retain builtin: true");
        assert!(merged_full.active, "active flag matches active ID");

        // Clear override (simulate reset)
        let reset_file = PresetFile {
            custom: Vec::new(),
            active: Some("full-power".to_string()),
            overrides: Vec::new(),
        };
        save_to(&root, &reset_file).unwrap();

        let reset_reloaded = load_from(&root).unwrap();
        let reset_listed = merged(&reset_reloaded);
        let restored_full = reset_listed.iter().find(|p| p.id == "full-power").unwrap();
        assert_eq!(restored_full.name, "Full Power");
        assert!(restored_full.all_skills && restored_full.all_mcps);
        assert_eq!(restored_full.icon, "zap");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn preset_store_does_not_follow_a_presets_symlink() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-presets-link-{}-{unique}",
            std::process::id()
        ));
        let store = root.join("store");
        fs::create_dir_all(&store).unwrap();
        let outside = root.join("outside.json");
        let original = "{\n  \"custom\": [{\n    \"id\": \"ssh-private\",\n    \"name\": \"Stolen\"\n  }],\n  \"active\": \"ssh-private\"\n}\n";
        fs::write(&outside, original).unwrap();
        let link = store.join("presets.json");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let loaded = load_from(&store);
        assert!(
            loaded.is_err(),
            "preset store must not read through the symlink: {loaded:?}"
        );

        let saved = save_to(
            &store,
            &PresetFile {
                custom: Vec::new(),
                active: Some("full-power".to_string()),
                overrides: Vec::new(),
            },
        );
        assert!(
            saved.is_err(),
            "preset store must not replace the symlink: {saved:?}"
        );
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the presets symlink must stay"
        );
        assert_eq!(fs::read_to_string(&outside).unwrap(), original);
        let _ = fs::remove_dir_all(&root);
    }
}
