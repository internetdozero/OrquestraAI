//! Deploy and selection tests. Split out of the MCP suite to stay under the file cap.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value as JsonValue;

use super::super::super::catalog::*;
use super::super::super::deploy::*;
use super::super::is_target;
#[cfg(unix)]
use super::mode_of;
use super::{server, TestHome};

#[cfg(unix)]
#[test]
fn write_never_loosens_a_mode_and_tightens_files_holding_secrets() {
    let test = TestHome::new("modes");

    let private = test.write("private.json", "old");
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(!write_config(&private, "new", false).unwrap());
    assert_eq!(mode_of(&private), 0o600);

    let shared = test.write("shared.json", "old");
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        !write_config(&shared, "new", false).unwrap(),
        "no secret, no tightening"
    );
    assert_eq!(mode_of(&shared), 0o644);

    let leaked = test.write("leaked.json", "old");
    fs::set_permissions(&leaked, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        write_config(&leaked, "new", true).unwrap(),
        "secret, tighten"
    );
    assert_eq!(mode_of(&leaked), 0o600);

    // A brand new file with secrets is private from the start.
    let fresh = test.home.join("fresh.json");
    write_config(&fresh, "{\"a\":1}", true).unwrap();
    assert_eq!(mode_of(&fresh), 0o600);

    test.cleanup();
}

#[test]
fn global_selection_sets_the_baseline_but_keeps_harness_opt_ins() {
    let test = TestHome::new("global-select");
    test.paths.ensure_layout().unwrap();
    let seed = test.seed_file();

    let mut catalog = load_catalog(&test.paths, &seed).unwrap();
    catalog.mcps.get_mut("github").unwrap().enabled_harnesses = vec!["codex".to_string()];
    save_catalog(&test.paths, &catalog).unwrap();

    // A preset that drops everything disables the Global baseline but
    // must not wipe the harness opt-in.
    assert_eq!(apply_global_selection_at(&test.paths, &[]).unwrap(), 2);

    let reloaded = load_catalog(&test.paths, &seed).unwrap();
    for server in reloaded.mcps.values() {
        assert!(
            !server.global_enabled,
            "{} should be off globally",
            server.id
        );
    }
    assert_eq!(
        reloaded.mcps["github"].enabled_harnesses,
        vec!["codex".to_string()],
        "the codex opt-in survives the preset"
    );

    // Re-enabling one server reports exactly one change and still
    // leaves the opt-in alone.
    assert_eq!(
        apply_global_selection_at(&test.paths, &["headroom".to_string()]).unwrap(),
        1
    );
    let reloaded = load_catalog(&test.paths, &seed).unwrap();
    assert!(reloaded.mcps["headroom"].global_enabled);
    assert_eq!(
        reloaded.mcps["github"].enabled_harnesses,
        vec!["codex".to_string()]
    );

    test.cleanup();
}

#[test]
fn harnesses_without_an_adapter_are_rejected() {
    assert!(is_target("grok"));
    assert!(!is_target("hermes"));
    assert!(is_target("freebuff"));
    assert!(is_target("codex"));
    assert!(is_target("opencode"));

    assert!(validate_server(&server("ok-id", "npx", &[])).is_ok());
    assert!(validate_server(&server("bad id", "npx", &[])).is_err());
    assert!(validate_server(&server("../escape", "npx", &[])).is_err());
    assert!(validate_server(&server("ok", "  ", &[])).is_err());
}

#[cfg(unix)]
#[test]
fn deploy_does_not_read_a_symlinked_harness_config() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = orquestra_core::test_support::root().join(format!(
        "orquestra-mcp-config-link-{}-{unique}",
        std::process::id()
    ));
    let home = base.join("home");
    let root = base.join("orquestra");
    fs::create_dir_all(home.join(".config/opencode")).unwrap();
    let outside = base.join("outside.jsonc");
    fs::write(&outside, "{\n  \"secret\": \"ssh-private\"\n}\n").unwrap();
    let config = home.join(".config/opencode/opencode.jsonc");
    std::os::unix::fs::symlink(&outside, &config).unwrap();
    let seed = base.join("seed.json");
    fs::write(
        &seed,
        r#"{
  "headroom": {
    "id": "headroom",
    "name": "Headroom",
    "description": "local",
    "command": "headroom",
    "args": ["mcp"],
    "env": {},
    "enabled": true
  }
}
"#,
    )
    .unwrap();
    let paths = crate::paths::OrquestraPaths {
        vault: root.join("skills-library"),
        active: root.join("active"),
        root,
    };

    let _ = deploy_at(&home, &paths, &seed);
    assert!(
        fs::symlink_metadata(&config)
            .unwrap()
            .file_type()
            .is_symlink(),
        "deploy must not replace the config symlink"
    );
    assert_eq!(
        fs::read_to_string(&outside).unwrap(),
        "{\n  \"secret\": \"ssh-private\"\n}\n"
    );
    let _ = fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn catalog_does_not_read_a_symlinked_mcps_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = orquestra_core::test_support::root().join(format!(
        "orquestra-mcp-catalog-link-{}-{unique}",
        std::process::id()
    ));
    let root = base.join("orquestra");
    fs::create_dir_all(&root).unwrap();
    let outside = base.join("outside.json");
    let original = "{\n  \"mcps\": {\n    \"ssh-private\": {\n      \"id\": \"ssh-private\",\n      \"name\": \"Stolen\",\n      \"command\": \"ssh-private\"\n    }\n  }\n}\n";
    fs::write(&outside, original).unwrap();
    let link = root.join("mcps.json");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let paths = crate::paths::OrquestraPaths {
        vault: root.join("skills-library"),
        active: root.join("active"),
        root: root.clone(),
    };
    let seed = base.join("missing-seed.json");

    let loaded = load_catalog(&paths, &seed);
    assert!(
        loaded.is_err(),
        "catalog must not read through the symlink: {loaded:?}"
    );
    let saved = save_catalog(&paths, &Catalog::default());
    assert!(
        saved.is_err(),
        "catalog must not replace the symlink: {saved:?}"
    );
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the catalog symlink must stay"
    );
    assert_eq!(fs::read_to_string(&outside).unwrap(), original);
    let _ = fs::remove_dir_all(&base);
}

/// Task #281: a symlinked seed file fails closed instead of seeding the
/// catalog from an attacker-chosen target.
#[cfg(unix)]
#[test]
fn seed_catalog_refuses_a_symlinked_seed_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = orquestra_core::test_support::root().join(format!(
        "orquestra-mcp-seed-link-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&base).unwrap();
    let outside = base.join("outside.json");
    fs::write(
        &outside,
        "{\"planted\":{\"id\":\"planted\",\"name\":\"Planted\",\"command\":\"evil\"}}",
    )
    .unwrap();
    let link = base.join("seed.json");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    assert!(
        seed_catalog(&link).is_err(),
        "a symlinked seed must not plant servers"
    );
    let _ = fs::remove_dir_all(&base);
}

/// Task #282: `mcps.json` on disk must never hold a readable token — sealed
/// envelope only — while the catalog still round-trips the values.
#[test]
fn catalog_file_on_disk_holds_no_plaintext_tokens() {
    let test = TestHome::new("sealed");
    let mut server = server("gh", "npx", &[]);
    server
        .env
        .insert("GITHUB_TOKEN".to_string(), "ghp-very-secret".to_string());
    let mut catalog = Catalog::default();
    catalog.mcps.insert("gh".to_string(), server);
    save_catalog(&test.paths, &catalog).expect("save sealed");
    let raw = fs::read_to_string(test.paths.root.join("mcps.json")).expect("raw catalog");
    assert!(
        !raw.contains("ghp-very-secret"),
        "token leaked to disk in plaintext"
    );
    assert!(
        raw.contains("orquestraSealed"),
        "expected a sealed envelope on disk"
    );
    let reloaded =
        load_catalog(&test.paths, std::path::Path::new("missing-seed.json")).expect("reload");
    assert_eq!(
        reloaded.mcps["gh"].env["GITHUB_TOKEN"], "ghp-very-secret",
        "sealed round-trip must restore the token"
    );
    test.cleanup();
}

#[cfg(unix)]
mod deploy_matrix;
