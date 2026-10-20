#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Unit tests for seeding, the adapter serializers, and the deploy's
//! non-destructive guarantees.

use super::catalog::*;
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
            url: None,
            headers: HashMap::new(),
            global_enabled: true,
            enabled_harnesses: Vec::new(),
            official: false,
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

    mod deploy_cases;

}
