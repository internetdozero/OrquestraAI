//! Full adapter deployment matrix with preservation and permissions.

use super::*;

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
    test.mkdir(".freebuff");
    test.mkdir(".agents");
    test.write(
        ".agents/mcp.json",
        r#"{
  "mcpServers": { "hand-added": { "command": "custom" } },
  "theme": "dark"
}
"#,
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
        9,
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
    let gemini: JsonValue = serde_json::from_str(&test.read(".gemini/settings.json")).unwrap();
    assert_eq!(gemini["mcpServers"]["github"]["command"], "npx");

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

    let freebuff: JsonValue = serde_json::from_str(&test.read(".agents/mcp.json")).unwrap();
    assert_eq!(freebuff["theme"], "dark");
    assert_eq!(freebuff["mcpServers"]["hand-added"]["command"], "custom");
    assert_eq!(freebuff["mcpServers"]["github"]["command"], "npx");
    assert_eq!(freebuff["mcpServers"]["headroom"]["command"], "headroom");

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
    assert_eq!(changed.len(), 9);

    let claude: JsonValue = serde_json::from_str(&test.read(".claude.json")).unwrap();
    assert!(claude["mcpServers"].get("github").is_none());
    assert!(claude["mcpServers"].get("headroom").is_some());
    assert_eq!(claude["numStartups"], 7);

    let antigravity: JsonValue =
        serde_json::from_str(&test.read(".gemini/config/mcp_config.json")).unwrap();
    assert!(antigravity["mcpServers"].get("github").is_none());
    assert_eq!(antigravity["mcpServers"]["hand-added"]["command"], "custom");
    assert_eq!(antigravity["theme"], "dark");
    let gemini: JsonValue = serde_json::from_str(&test.read(".gemini/settings.json")).unwrap();
    assert!(gemini["mcpServers"].get("github").is_none());

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

    let freebuff: JsonValue = serde_json::from_str(&test.read(".agents/mcp.json")).unwrap();
    assert!(freebuff["mcpServers"].get("github").is_none());
    assert!(freebuff["mcpServers"].get("headroom").is_some());
    assert_eq!(freebuff["mcpServers"]["hand-added"]["command"], "custom");
    assert_eq!(freebuff["theme"], "dark");

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
