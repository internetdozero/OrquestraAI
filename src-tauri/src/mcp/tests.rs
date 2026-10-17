//! Unit tests for seeding, the adapter serializers, and the deploy's
//! non-destructive guarantees.

use super::catalog::*;
use super::deploy::*;
use super::serializers::*;
use super::*;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::Value as JsonValue;

use crate::harness::is_mcp_target as is_target;
use crate::paths::OrquestraPaths;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestHome {
        home: PathBuf,
        paths: OrquestraPaths,
    }

    impl TestHome {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let base = std::env::temp_dir().join(format!(
                "orquestra-mcp-{label}-{}-{unique}",
                std::process::id()
            ));
            let home = base.join("home");
            let root = base.join("orquestra");

            fs::create_dir_all(&home).unwrap();

            TestHome {
                home,
                paths: OrquestraPaths {
                    vault: root.join("skills-library"),
                    active: root.join("active"),
                    root,
                },
            }
        }

        fn write(&self, rel: &str, content: &str) -> PathBuf {
            let path = self.home.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
            path
        }

        fn mkdir(&self, rel: &str) {
            fs::create_dir_all(self.home.join(rel)).unwrap();
        }

        fn read(&self, rel: &str) -> String {
            fs::read_to_string(self.home.join(rel)).unwrap()
        }

        /// A catalog with one token-bearing server and one plain server, in the
        /// shape the audited dataset uses.
        fn seed_file(&self) -> PathBuf {
            self.write(
                "seed.json",
                r#"{
  "github": {
    "id": "github",
    "name": "GitHub MCP Server",
    "description": "Repository inspection.",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"],
    "env": { "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_test" },
    "enabled": true
  },
  "headroom": {
    "id": "headroom",
    "name": "Headroom Token Compressor",
    "description": "Compress context via a local proxy.",
    "command": "headroom",
    "args": ["mcp", "serve", "--proxy-url", "http://127.0.0.1:8788"],
    "env": {},
    "enabled": true
  }
}
"#,
            )
        }

        fn cleanup(&self) {
            let _ = fs::remove_dir_all(self.home.parent().unwrap());
        }
    }

    fn server(id: &str, command: &str, args: &[&str]) -> McpServer {
        McpServer {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            command: command.to_string(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            env: HashMap::new(),
            global_enabled: true,
            enabled_harnesses: Vec::new(),
        }
    }

    fn with_env(mut server: McpServer, key: &str, value: &str) -> McpServer {
        server.env.insert(key.to_string(), value.to_string());
        server
    }

    fn mode_of(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn seeds_the_catalog_from_the_audited_dataset() {
        let test = TestHome::new("seed");
        let catalog = seed_catalog(&test.seed_file()).unwrap();

        assert_eq!(catalog.mcps.len(), 2);
        assert!(catalog.managed_ids.is_empty());

        let github = &catalog.mcps["github"];
        assert!(github.global_enabled, "audited `enabled: true` maps to global");
        assert_eq!(github.command, "npx");
        assert_eq!(github.args, vec!["-y", "@modelcontextprotocol/server-github"]);
        assert!(github.enabled_harnesses.is_empty());
        assert_eq!(github.env["GITHUB_PERSONAL_ACCESS_TOKEN"], "ghp_test");

        // A missing seed file leaves the catalog empty rather than failing.
        assert_eq!(
            seed_catalog(&test.home.join("absent.json"))
                .unwrap()
                .mcps
                .len(),
            0
        );

        test.cleanup();
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
        let value: JsonValue = serde_json::from_str(&rendered).unwrap();

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
        let value: JsonValue = serde_json::from_str(existing).unwrap();
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

        let mut headroom = server(
            "headroom",
            "headroom",
            &["mcp", "serve", "--proxy-url", "http://127.0.0.1:8788"],
        );
        headroom.env.clear();

        let rendered =
            render_codex_toml(existing, &[headroom], &BTreeSet::new()).unwrap();

        // Untouched content, in place.
        assert!(rendered.contains("# hand-written comment"));
        assert!(rendered.contains("model = \"gpt-5\""));
        assert!(rendered.contains("[mcp_servers.node_repl]"));
        assert!(rendered.contains("startup_timeout_sec = 120"));
        assert!(rendered.contains("[hooks.state.\"hooks.json:interrupt:0:0\"]"));

        // Our keys land under the table Codex actually reads.
        assert!(rendered.contains("[mcp_servers.headroom]"));
        assert!(rendered.contains("--proxy-url"));
        assert!(!rendered.contains("[mcp."), "never the unsupported [mcp] table");

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
        assert_eq!(changed.len(), 7, "every installed target is written: {changed:?}");
        assert!(
            changed.iter().any(|path| path.contains("kilo.jsonc") && path.contains("0600")),
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
        assert_eq!(
            antigravity["mcpServers"]["hand-added"]["command"],
            "custom"
        );

        let kilo: JsonValue = serde_json::from_str(&test.read(".config/kilo/kilo.jsonc")).unwrap();
        assert_eq!(kilo["permission"]["bash"], "allow");
        assert_eq!(kilo["skills"]["paths"][0], "/projection");
        assert_eq!(kilo["mcp"]["github"]["type"], "local");
        assert_eq!(kilo["mcp"]["github"]["environment"]["GITHUB_PERSONAL_ACCESS_TOKEN"], "ghp_test");
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
        assert!(deploy_at(&test.home, &test.paths, &seed).unwrap().is_empty());
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
        assert!(!write_config(&shared, "new", false).unwrap(), "no secret, no tightening");
        assert_eq!(mode_of(&shared), 0o644);

        let leaked = test.write("leaked.json", "old");
        fs::set_permissions(&leaked, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(write_config(&leaked, "new", true).unwrap(), "secret, tighten");
        assert_eq!(mode_of(&leaked), 0o600);

        // A brand new file with secrets is private from the start.
        let fresh = test.home.join("fresh.json");
        write_config(&fresh, "{\"a\":1}", true).unwrap();
        assert_eq!(mode_of(&fresh), 0o600);

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
}
