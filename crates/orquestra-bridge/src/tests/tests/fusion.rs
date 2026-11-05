//! Full fusion integration test: adopt, back up and link harness state.

use std::fs;
use std::path::PathBuf;

use super::super::super::*;
use super::{assert_linked, TestHome};

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

    let original_alpha =
        fs::read_to_string(test.home.join(".gemini/config/skills/alpha/SKILL.md")).unwrap();

    let report = deploy_at(&test.home, &test.paths).unwrap();

    let backup = PathBuf::from(report.backup_dir.clone().unwrap());
    assert!(backup
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("pre-fusion-"));

    // Originals are preserved in the backup.
    assert!(backup
        .join("antigravity/.gemini__config__skills/alpha/SKILL.md")
        .is_file());
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
    assert!(test
        .paths
        .projection_dir("codex")
        .join(".system/imagegen/SKILL.md")
        .is_file());
    assert!(test
        .paths
        .projection_dir("hermes")
        .join(".curator_state")
        .is_file());

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
    assert_linked(
        &test.home.join(".codex/AGENTS.md"),
        &test.paths.agents_path(),
    );
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
    assert_eq!(
        fs::read_to_string(test.paths.vault.join("alpha/SKILL.md")).unwrap(),
        original_alpha,
        "the harness's original skill replaces an older same-name vault copy"
    );
    assert_linked(
        &test.paths.projection_dir("hermes").join("nested"),
        &test.paths.vault.join("nested"),
    );

    // Kilo config keeps its comment and gains the projection path.
    let kilo = fs::read_to_string(test.home.join(".config/kilo/kilo.jsonc")).unwrap();
    assert!(kilo.contains("// keep me"));
    assert!(kilo.contains(
        &test
            .paths
            .projection_dir("kilo")
            .to_string_lossy()
            .into_owned()
    ));

    // OpenCode config is created for an installed harness.
    let opencode = fs::read_to_string(test.home.join(".config/opencode/opencode.jsonc")).unwrap();
    assert!(opencode.contains(
        &test
            .paths
            .projection_dir("opencode")
            .to_string_lossy()
            .into_owned()
    ));

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
