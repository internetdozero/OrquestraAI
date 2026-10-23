//! Freebuff global instruction locations: fusion must link whichever of
//! `~/.freebuff/AGENTS.md` or `~/.config/freebuff/AGENTS.md` the operator's
//! platform created, preserve any existing content, and stay a no-op rerun.

use std::fs;

use super::super::super::*;
use super::{assert_linked, TestHome};

fn freebuff_status(report: &BridgeReport) -> &HarnessStatus {
    report
        .harnesses
        .iter()
        .find(|status| status.id == "freebuff")
        .expect("freebuff status")
}

/// Install-root variant: `~/.freebuff/AGENTS.md` links to the master file and
/// the absent XDG location does not block bridging.
#[test]
fn freebuff_links_install_root_instructions() {
    let test = TestHome::new("freebuff-install-root");

    test.mkdir(".freebuff/worktrees");
    test.write(".freebuff/AGENTS.md", "Freebuff house rules\n");
    test.mkdir(".config/manicode");

    let report = deploy_at(&test.home, &test.paths).unwrap();
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);

    let master = test.paths.agents_path();
    assert_linked(&test.home.join(".freebuff/AGENTS.md"), &master);
    assert_linked(&test.home.join(".config/manicode/AGENTS.md"), &master);

    let status = freebuff_status(&report);
    assert!(status.instructions_linked);
    assert!(status.bridged, "freebuff should bridge: {status:?}");

    // Existing content is folded into the master file, not dropped.
    let body = fs::read_to_string(&master).unwrap();
    assert!(body.contains("Freebuff house rules"));

    test.cleanup();
}

/// XDG variant: `~/.config/freebuff/AGENTS.md` links when that platform's
/// directory exists.
#[test]
fn freebuff_links_xdg_config_instructions() {
    let test = TestHome::new("freebuff-xdg");

    test.mkdir(".freebuff/worktrees");
    test.mkdir(".config/freebuff");
    test.write(".config/freebuff/AGENTS.md", "Freebuff XDG rules\n");
    test.mkdir(".config/manicode");

    let report = deploy_at(&test.home, &test.paths).unwrap();
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);

    let master = test.paths.agents_path();
    assert_linked(&test.home.join(".config/freebuff/AGENTS.md"), &master);

    let status = freebuff_status(&report);
    assert!(status.instructions_linked);
    assert!(status.bridged, "freebuff should bridge: {status:?}");

    let body = fs::read_to_string(&master).unwrap();
    assert!(body.contains("Freebuff XDG rules"));

    test.cleanup();
}

/// Pre-existing instruction files survive in the run backup and reach the
/// master file exactly once.
#[test]
fn freebuff_instruction_content_is_preserved() {
    let test = TestHome::new("freebuff-preserve");

    test.mkdir(".freebuff/worktrees");
    test.write(".freebuff/AGENTS.md", "Freebuff house rules\n");
    test.mkdir(".config/manicode");
    test.write(".config/manicode/AGENTS.md", "Manicode house rules\n");

    let report = deploy_at(&test.home, &test.paths).unwrap();
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);

    let backup = std::path::PathBuf::from(report.backup_dir.unwrap());
    assert_eq!(
        fs::read_to_string(backup.join("freebuff/.freebuff__AGENTS.md")).unwrap(),
        "Freebuff house rules\n"
    );
    assert_eq!(
        fs::read_to_string(backup.join("freebuff/.config__manicode__AGENTS.md")).unwrap(),
        "Manicode house rules\n"
    );

    let body = fs::read_to_string(test.paths.agents_path()).unwrap();
    assert!(body.contains("Freebuff house rules"));
    assert!(body.contains("Manicode house rules"));

    test.cleanup();
}

/// A second deploy is a no-op: no new backup, no errors, links intact.
#[test]
fn freebuff_instructions_setup_is_idempotent() {
    let test = TestHome::new("freebuff-idempotent");

    test.mkdir(".freebuff/worktrees");
    test.write(".freebuff/AGENTS.md", "Freebuff house rules\n");
    test.mkdir(".config/manicode");

    let first = deploy_at(&test.home, &test.paths).unwrap();
    assert!(first.backup_dir.is_some());
    assert!(first.errors.is_empty());

    let before = fs::read_to_string(test.paths.agents_path()).unwrap();

    let second = deploy_at(&test.home, &test.paths).unwrap();
    assert!(second.backup_dir.is_none(), "second run must be a no-op");
    assert!(second.errors.is_empty(), "errors: {:?}", second.errors);
    assert_eq!(freebuff_status(&second).bridged, true);

    assert_linked(
        &test.home.join(".freebuff/AGENTS.md"),
        &test.paths.agents_path(),
    );
    assert_eq!(
        fs::read_to_string(test.paths.agents_path()).unwrap(),
        before,
        "master file must not gain duplicates on rerun"
    );

    test.cleanup();
}
