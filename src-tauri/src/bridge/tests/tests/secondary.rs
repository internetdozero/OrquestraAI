//! Secondary-discovery fusion tests, split out to stay under the file cap.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::super::*;
use super::{assert_linked, TestHome};
use crate::paths::OrquestraPaths;

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
    test.skill(
        ".codex/vendor_imports/skills/skills/.curated",
        "curated-one",
    );
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
    test.skill(
        ".hermes/hermes-agent/optional-skills/security",
        "threat-model",
    );

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
    assert!(report
        .quarantined
        .contains(&".grok/bundled/skills".to_string()));
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

#[cfg(unix)]
#[test]
fn fusion_does_not_adopt_through_a_builtin_directory_symlink() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-bridge-builtin-link-{}-{unique}",
        std::process::id()
    ));
    let home = base.join("home");
    let root = base.join("orquestra");
    let outside = base.join("outside");
    fs::create_dir_all(home.join(".gemini/config")).unwrap();
    fs::create_dir_all(home.join(".gemini/antigravity")).unwrap();
    fs::create_dir_all(outside.join("skills/stolen")).unwrap();
    fs::write(
        outside.join("skills/stolen/SKILL.md"),
        "---\nname: stolen\ndescription: stolen skill.\n---\n\nssh-private\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, home.join(".gemini/antigravity/builtin")).unwrap();

    let paths = OrquestraPaths {
        vault: root.join("skills-library"),
        active: root.join("active"),
        root,
    };
    let report = deploy_at(&home, &paths).unwrap();
    let copied = paths.vault.join("stolen/SKILL.md");
    assert!(
        !copied.exists(),
        "fusion copied the skill through the builtin symlink: {report:?}"
    );
    assert!(
        fs::read_to_string(outside.join("skills/stolen/SKILL.md"))
            .unwrap()
            .contains("ssh-private"),
        "the outside skill must stay put"
    );
    let _ = fs::remove_dir_all(&base);
}
