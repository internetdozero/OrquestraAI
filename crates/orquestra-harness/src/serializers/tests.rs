//! Dialect tests: foreign keys survive, managed keys are ours, and a config
//! already in canonical form round-trips byte for byte.

use super::json::render_pretty;
use super::*;
use crate::server::HarnessServer;
use std::collections::BTreeSet;

fn server(id: &str, command: &str, args: &[&str]) -> HarnessServer {
    HarnessServer {
        args: args.iter().map(|arg| arg.to_string()).collect(),
        ..HarnessServer::stdio(id, command)
    }
}

fn with_env(mut server: HarnessServer, key: &str, value: &str) -> HarnessServer {
    server.env.insert(key.to_string(), value.to_string());
    server
}

#[test]
fn json_envelopes_keep_foreign_keys_and_entries() {
    let existing = r#"{
  "numStartups": 42,
  "projects": { "/home/system/repo": { "history": ["a"] } },
  "mcpServers": {
    "hand-added": { "command": "custom-tool" },
    "github": { "command": "stale", "args": [], "tools": ["*"] },
    "gone": { "command": "x" }
  }
}"#;

    let active = vec![with_env(
        server("github", "npx", &["-y", "server-github"]),
        "TOKEN",
        "secret",
    )];
    let stale: BTreeSet<String> = ["gone".to_string()].into_iter().collect();

    let rendered = render_mcp_servers(existing, &active, &stale, Some("stdio")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();

    // Claude's own settings are untouched.
    assert_eq!(value["numStartups"], 42);
    assert_eq!(value["projects"]["/home/system/repo"]["history"][0], "a");

    // A server the catalog has never owned is left alone, and the id we
    // used to own is cleaned up.
    assert_eq!(value["mcpServers"]["hand-added"]["command"], "custom-tool");
    assert!(value["mcpServers"].get("gone").is_none());

    // Managed keys are ours, foreign keys on the same entry survive.
    assert_eq!(value["mcpServers"]["github"]["command"], "npx");
    assert_eq!(value["mcpServers"]["github"]["type"], "stdio");
    assert_eq!(value["mcpServers"]["github"]["tools"][0], "*");
    assert_eq!(value["mcpServers"]["github"]["env"]["TOKEN"], "secret");

    // With nothing to add or remove, a document already in canonical form
    // round-trips byte for byte: no write, no backup, no churn.
    let value: serde_json::Value = serde_json::from_str(existing).unwrap();
    let canonical = render_pretty(&value).unwrap();
    let untouched = render_mcp_servers(&canonical, &[], &BTreeSet::new(), None).unwrap();
    assert_eq!(untouched, canonical);
}

#[test]
fn jsonc_targets_use_the_mcp_key_merged_command_and_environment() {
    let existing = r#"{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "stale-shape": { "command": "old", "args": ["one"] },
    "gone": { "command": "old" }
  },
  "skills": { "paths": ["/home/system/.orquestra/active/opencode"] }
}"#;

    let active = vec![with_env(
        server("github", "npx", &["-y", "server-github"]),
        "TOKEN",
        "secret",
    )];
    let stale: BTreeSet<String> = ["gone".to_string()].into_iter().collect();

    let rendered = render_jsonc_block(existing, &active, &stale).unwrap();
    let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();

    // The envelope is `mcp`, never `mcpServers`.
    assert!(value.get("mcpServers").is_none());
    assert!(value.get("mcp").is_some());

    // The bridge's own `skills.paths` entry survives.
    assert_eq!(
        value["skills"]["paths"][0],
        "/home/system/.orquestra/active/opencode"
    );
    assert!(value["mcp"].get("gone").is_none());

    let github = &value["mcp"]["github"];
    assert_eq!(github["type"], "local");
    assert_eq!(github["enabled"], true);
    assert_eq!(
        github["command"],
        serde_json::json!(["npx", "-y", "server-github"])
    );
    assert_eq!(github["environment"]["TOKEN"], "secret");
    // The merged-array shape must not also carry a separate args key.
    assert!(github.get("args").is_none());
}

#[test]
fn codex_writes_mcp_servers_tables_and_keeps_the_rest_of_the_file() {
    let existing = r#"# hand-written comment
model = "gpt-5"

[mcp_servers.node_repl]
command = "/opt/codex-desktop/bin/node_repl"
args = []
startup_timeout_sec = 120

[mcp_servers.headroom]
command = "/home/system/.local/bin/headroom"
args = ["mcp", "serve"]

[mcp_servers.headroom.env]
HEADROOM_PROXY_URL = "http://127.0.0.1:8788"

[hooks.state."hooks.json:interrupt:0:0"]
enabled = true
"#;

    let headroom = server(
        "headroom",
        "headroom",
        &["mcp", "serve", "--proxy-url", "http://127.0.0.1:8788"],
    );

    let rendered = render_codex_toml(existing, &[headroom], &BTreeSet::new()).unwrap();

    // Untouched content, in place.
    assert!(rendered.contains("# hand-written comment"));
    assert!(rendered.contains("model = \"gpt-5\""));
    assert!(rendered.contains("[mcp_servers.node_repl]"));
    assert!(rendered.contains("startup_timeout_sec = 120"));
    assert!(rendered.contains("[hooks.state.\"hooks.json:interrupt:0:0\"]"));

    // Our keys land under the table Codex actually reads.
    assert!(rendered.contains("[mcp_servers.headroom]"));
    assert!(rendered.contains("--proxy-url"));
    assert!(
        !rendered.contains("[mcp."),
        "never the unsupported [mcp] table"
    );

    // A hand-added env var on a managed server survives an empty catalog
    // env, and the result still parses as TOML.
    assert!(rendered.contains("HEADROOM_PROXY_URL"));
    let _: toml_edit::DocumentMut = rendered.parse().expect("output must be valid TOML");

    // `node_repl` is not in the catalog, so disabling everything we manage
    // must leave it and the rest of the file alone.
    let emptied = render_codex_toml(
        &rendered,
        &[],
        &["headroom".to_string()].into_iter().collect(),
    )
    .unwrap();
    assert!(!emptied.contains("[mcp_servers.headroom]"));
    assert!(emptied.contains("[mcp_servers.node_repl]"));
    assert!(emptied.contains("# hand-written comment"));
    assert!(emptied.contains("[hooks.state.\"hooks.json:interrupt:0:0\"]"));
}

/// The registry's envelope decides the dialect, so a harness cannot silently
/// receive another harness's shape.
#[test]
fn dispatch_follows_the_harness_format() {
    let active = vec![server("github", "npx", &["-y", "server-github"])];
    let stale = BTreeSet::new();

    let claude = render_mcp_config(
        orquestra_core::harness::McpFormat::JsonMcpServers,
        "{}",
        &active,
        &stale,
        None,
    )
    .unwrap();
    assert!(claude.contains("\"mcpServers\""));

    let opencode = render_mcp_config(
        orquestra_core::harness::McpFormat::JsoncMcpBlock,
        "{}",
        &active,
        &stale,
        None,
    )
    .unwrap();
    assert!(opencode.contains("\"mcp\""));
    assert!(!opencode.contains("\"mcpServers\""));

    let codex = render_mcp_config(
        orquestra_core::harness::McpFormat::CodexToml,
        "",
        &active,
        &stale,
        None,
    )
    .unwrap();
    assert!(codex.contains("[mcp_servers.github]"));
}

#[test]
fn codex_toml_writes_remote_headers_under_http_headers() {
    let remote = HarnessServer {
        url: Some("https://mcp.example.test/mcp".to_string()),
        headers: [("Authorization".to_string(), "Bearer token".to_string())]
            .into_iter()
            .collect(),
        ..server("remote", "placeholder", &[])
    };
    let rendered = render_codex_toml("", &[remote], &BTreeSet::new()).unwrap();
    let document: toml_edit::DocumentMut = rendered.parse().unwrap();
    let entry = &document["mcp_servers"]["remote"];
    assert_eq!(entry["url"].as_str(), Some("https://mcp.example.test/mcp"));
    assert_eq!(
        entry["http_headers"]["Authorization"].as_str(),
        Some("Bearer token")
    );
    assert!(entry.get("headers").is_none());
}

/// A remote server drops `command`/`args` in both JSON shapes, so a harness
/// never spawns a placeholder process for an HTTP endpoint.
#[test]
fn remote_servers_replace_the_stdio_fields() {
    let remote = HarnessServer {
        url: Some("https://mcp.example.test/mcp".to_string()),
        ..server("remote", "npx", &["-y", "placeholder"])
    };
    let active = [remote];

    let claude = render_mcp_servers("", &active, &BTreeSet::new(), Some("stdio")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&claude).unwrap();
    assert_eq!(value["mcpServers"]["remote"]["type"], "http");
    assert_eq!(
        value["mcpServers"]["remote"]["url"],
        "https://mcp.example.test/mcp"
    );
    assert!(value["mcpServers"]["remote"].get("command").is_none());

    let opencode = render_jsonc_block("", &active, &BTreeSet::new()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&opencode).unwrap();
    assert_eq!(value["mcp"]["remote"]["type"], "remote");
    assert!(value["mcp"]["remote"].get("command").is_none());
}
