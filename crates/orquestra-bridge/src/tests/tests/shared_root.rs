//! `~/.agents/skills` ownership: the Codebuff SDK's canonical global root.

use std::fs;

use super::super::super::*;
use super::{assert_linked, TestHome};

#[test]
fn shared_agent_root_is_freebuffs_projection() {
    let test = TestHome::new("shared-root");

    // Codex and Grok are installed and also inject the shared root.
    test.mkdir(".codex/skills");
    test.mkdir(".grok/skills");
    test.mkdir(".freebuff");
    test.mkdir(".config/manicode");
    test.skill(".agents/skills", "orca-cli");
    test.write(".agents/.skill-lock.json", "{\n  \"version\": 3\n}\n");

    let report = deploy_at(&test.home, &test.paths).unwrap();

    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
    assert_linked(
        &test.home.join(".agents/skills"),
        &test.paths.projection_dir("freebuff"),
    );
    // The skill that lived behind the shared root stays reachable through the
    // projection instead of disappearing with the folder.
    assert_linked(
        &test.paths.projection_dir("freebuff").join("orca-cli"),
        &test.paths.vault.join("orca-cli"),
    );
    assert!(test.paths.vault.join("orca-cli/SKILL.md").is_file());

    // The owner links the path, so the other readers treat it as cleared, and
    // no harness may empty it as a secondary backdoor.
    for id in ["freebuff", "codex", "grok"] {
        let status = report
            .harnesses
            .iter()
            .find(|status| status.id == id)
            .unwrap_or_else(|| panic!("{id} is registered"));
        assert!(status.bridged, "{id} was not bridged: {status:?}");
        assert!(status.secondary_cleared, "{id} kept a backdoor: {status:?}");
    }
    assert!(!report.quarantined.contains(&".agents/skills".to_string()));
    assert!(test.home.join(".agents/.skill-lock.json").is_file());

    // Idempotent: the link is the steady state, never re-archived.
    let second = deploy_at(&test.home, &test.paths).unwrap();
    assert!(second.backup_dir.is_none());
    assert!(second.adopted.is_empty());
    assert!(second.quarantined.is_empty());
    assert!(second
        .harnesses
        .iter()
        .filter(|status| status.installed)
        .all(|status| status.bridged));

    test.cleanup();
}

#[test]
fn shared_agent_root_stays_quarantined_without_freebuff() {
    let test = TestHome::new("shared-root-unowned");

    // Codex alone is installed, so nobody projects the shared root.
    test.mkdir(".codex/skills");
    test.skill(".agents/skills", "stray-audit");

    let report = deploy_at(&test.home, &test.paths).unwrap();

    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
    let root = test.home.join(".agents/skills");
    assert!(root.is_dir(), "the shared root must keep existing");
    assert!(
        fs::read_dir(&root).unwrap().next().is_none(),
        "the unowned shared root still feeds unmanaged skills"
    );
    assert!(report.quarantined.contains(&".agents/skills".to_string()));
    assert!(test.paths.vault.join("stray-audit/SKILL.md").is_file());

    test.cleanup();
}
