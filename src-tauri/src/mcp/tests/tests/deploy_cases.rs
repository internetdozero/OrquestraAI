//! Deploy and selection tests. Split out of the MCP suite to stay under the file cap.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value as JsonValue;

use super::super::super::catalog::*;
use super::super::super::deploy::*;
use super::super::is_target;
use super::{mode_of, server, TestHome};

#[test]
fn deploy_writes_every_adapter_and_removes_only_what_it_owns() {
    let test = TestHome::new("deploy");
    let seed = test.seed_file();

    // Every harness installed, each with the keys we must not break.
    test.mkdir(".gemini/config");
    test.write(
        ".gemini/config/mcp_config.json",
        r#"{
  "mcpServers": { "hand-added": { "command": "custom" } },
  "theme": "dark"
}
"#,
    );
    test.mkdir(".claude");
    test.write(
        ".claude.json",
        r#"{
  "numStartups": 7,
  "projects": { "/home/system/repo": { "history": ["x"] } },
  "mcpServers": {}
}
"#,
    );
    test.mkdir(".copilot");
    test.write(".copilot/mcp-config.json", "{\n  \"mcpServers\": {}\n}\n");
    test.mkdir(".config/opencode");
    test.write(
        ".config/opencode/opencode.jsonc",
        "{\n  \"skills\": { \"paths\": [\"/projection\"] }\n}\n",
    );
    test.mkdir(".config/kilo");
    test.write(
        ".config/kilo/kilo.jsonc",
        "{\n  \"permission\": { \"bash\": \"allow\" },\n  \"skills\": { \"paths\": [\"/projection\"] }\n}\n",
    );
    test.mkdir(".codex");
    test.write(
        ".codex/config.toml",
        "[mcp_servers.node_repl]\ncommand = \"/opt/node_repl\"\nargs = []\n",
    );
    test.mkdir(".grok");
    test.write(
        ".grok/config.toml",
        "[models]\ndefault = \"grok-4.7\"\n\n[mcp_servers.rea]\ncommand = \"/opt/rea\"\nargs = [\"mcp\"]\nstartup_timeout_sec = 30\n\n[plugins]\nenabled = [\"ponytail\"]\n",
    );

    // The two JSONC targets are world-readable to start with.
    fs::set_permissions(
        test.home.join(".config/kilo/kilo.jsonc"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();

    let changed = deploy_at(&test.home, &test.paths, &seed).unwrap();
    assert_eq!(
        changed.len(),
        7,
        "every installed target is written: {changed:?}"
    );
    assert!(
        changed
            .iter()
            .any(|path| path.contains("kilo.jsonc") && path.contains("0600")),
        "a config that gains a token is tightened: {changed:?}"
    );

    // Non-MCP content is intact everywhere.
    let claude: JsonValue = serde_json::from_str(&test.read(".claude.json")).unwrap();
    assert_eq!(claude["numStartups"], 7);
    assert_eq!(claude["projects"]["/home/system/repo"]["history"][0], "x");
    assert_eq!(claude["mcpServers"]["headroom"]["command"], "headroom");

    let antigravity: JsonValue =
        serde_json::from_str(&test.read(".gemini/config/mcp_config.json")).unwrap();
    assert_eq!(antigravity["theme"], "dark");
    assert_eq!(antigravity["mcpServers"]["hand-added"]["command"], "custom");

    let kilo: JsonValue = serde_json::from_str(&test.read(".config/kilo/kilo.jsonc")).unwrap();
    assert_eq!(kilo["permission"]["bash"], "allow");
    assert_eq!(kilo["skills"]["paths"][0], "/projection");
    assert_eq!(kilo["mcp"]["github"]["type"], "local");
    assert_eq!(
        kilo["mcp"]["github"]["environment"]["GITHUB_PERSONAL_ACCESS_TOKEN"],
        "ghp_test"
    );
    assert_eq!(mode_of(&test.home.join(".config/kilo/kilo.jsonc")), 0o600);

    let opencode: JsonValue =
        serde_json::from_str(&test.read(".config/opencode/opencode.jsonc")).unwrap();
    assert_eq!(opencode["skills"]["paths"][0], "/projection");

    let codex = test.read(".codex/config.toml");
    assert!(codex.contains("[mcp_servers.github]"));
    assert!(codex.contains("[mcp_servers.node_repl]"));
    let _: toml_edit::DocumentMut = codex.parse().expect("codex config must stay valid TOML");

    let grok = test.read(".grok/config.toml");
    assert!(grok.contains("[mcp_servers.github]"));
    assert!(grok.contains("[mcp_servers.headroom]"));
    assert!(grok.contains("[mcp_servers.rea]"));
    assert!(grok.contains("startup_timeout_sec = 30"));
    assert!(grok.contains("default = \"grok-4.7\""));
    assert!(grok.contains("ponytail"));
    let _: toml_edit::DocumentMut = grok.parse().expect("grok config must stay valid TOML");

    // Second run changes nothing.
    assert!(deploy_at(&test.home, &test.paths, &seed)
        .unwrap()
        .is_empty());
    assert_eq!(test.read(".codex/config.toml").trim(), codex.trim());

    // Disable one server globally: it disappears from every adapter while
    // the others and all foreign keys stay.
    let mut catalog = load_catalog(&test.paths, &seed).unwrap();
    catalog.mcps.get_mut("github").unwrap().global_enabled = false;
    save_catalog(&test.paths, &catalog).unwrap();

    let changed = deploy_at(&test.home, &test.paths, &seed).unwrap();
    assert_eq!(changed.len(), 7);

    let claude: JsonValue = serde_json::from_str(&test.read(".claude.json")).unwrap();
    assert!(claude["mcpServers"].get("github").is_none());
    assert!(claude["mcpServers"].get("headroom").is_some());
    assert_eq!(claude["numStartups"], 7);

    let antigravity: JsonValue =
        serde_json::from_str(&test.read(".gemini/config/mcp_config.json")).unwrap();
    assert!(antigravity["mcpServers"].get("github").is_none());
    assert_eq!(antigravity["mcpServers"]["hand-added"]["command"], "custom");
    assert_eq!(antigravity["theme"], "dark");

    let kilo: JsonValue = serde_json::from_str(&test.read(".config/kilo/kilo.jsonc")).unwrap();
    assert!(kilo["mcp"].get("github").is_none());
    assert_eq!(kilo["permission"]["bash"], "allow");

    let codex = test.read(".codex/config.toml");
    assert!(!codex.contains("[mcp_servers.github]"));
    assert!(codex.contains("[mcp_servers.headroom]"));
    assert!(codex.contains("[mcp_servers.node_repl]"));

    let grok = test.read(".grok/config.toml");
    assert!(!grok.contains("[mcp_servers.github]"));
    assert!(grok.contains("[mcp_servers.headroom]"));
    assert!(grok.contains("[mcp_servers.rea]"));
    assert!(grok.contains("startup_timeout_sec = 30"));
    assert!(grok.contains("[plugins]"));

    // The backup keeps the pre-deploy bytes and the original privacy.
    let backup = test.paths.root.join("backups");
    let runs: Vec<_> = fs::read_dir(&backup)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    assert!(!runs.is_empty());
    assert!(runs
        .iter()
        .any(|run| run.join("kilo/kilo.jsonc.bak").is_file()));

    test.cleanup();
}

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
    assert!(!is_target("freebuff"));
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
    let base = PathBuf::from("/var/tmp/testing").join(format!(
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
