//! Unit tests for the built-in profiles, the preset store and the planning
//! that decides what an apply will project.

use super::apply::{plan_mcps, plan_skills};
use super::builtins;
use super::store::*;
use super::*;
use std::fs;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
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

        assert_eq!(
            ids,
            vec!["stealth", "rust-dev", "web-fullstack", "full-power"]
        );
        assert!(all.iter().all(|preset| preset.builtin && !preset.active));

        let stealth = builtin("stealth");
        assert_eq!(stealth.icon, "shield");
        assert_eq!(stealth.skills, vec!["rtk"]);
        assert!(stealth.mcps.contains(&"headroom".to_string()));
        assert!(stealth.mcps.contains(&"orquestra-core".to_string()));
        assert!(!stealth.all_skills);

        assert_eq!(builtin("rust-dev").icon, "cpu");
        assert_eq!(builtin("web-fullstack").icon, "globe");

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
        assert_eq!(unique_id("Stealth", &file), "stealth-2");

        let mut taken = PresetFile::default();
        taken.custom.push(Preset {
            id: "bug-hunting".to_string(),
            name: "Bug Hunting".to_string(),
            ..builtin("stealth")
        });
        assert_eq!(unique_id("Bug Hunting", &taken), "bug-hunting-2");
    }

    #[test]
    fn plans_known_ids_and_warns_about_the_rest() {
        let (skills, warnings) = plan_skills(
            &builtin("rust-dev"),
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
            &builtin("rust-dev"),
            &["rtk".to_string(), "github".to_string()],
        );
        assert_eq!(skills.len(), 2);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("modular-start"));

        let (_, warnings) = plan_mcps(&builtin("rust-dev"), &["github".to_string()]);
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
            all_skills: false,
            all_mcps: false,
            builtin: false,
            active: false,
        };
        let file = PresetFile {
            custom: vec![custom.clone()],
            active: Some("game-dev".to_string()),
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
            .find(|preset| preset.id == "stealth")
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
}
