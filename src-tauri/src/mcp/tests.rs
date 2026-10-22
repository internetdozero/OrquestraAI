#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Unit tests for seeding, catalog storage, and the deploy's non-destructive
//! guarantees. The dialect tests live with the serializers in
//! `orquestra-harness`.

use super::catalog::*;
use super::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

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

    #[cfg(unix)]
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
        assert!(
            github.global_enabled,
            "audited `enabled: true` maps to global"
        );
        assert_eq!(github.command, "npx");
        assert_eq!(
            github.args,
            vec!["-y", "@modelcontextprotocol/server-github"]
        );
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
    fn seed_defaults_to_a_file_inside_the_storage_root() {
        // The operator override is process-global; only assert the default
        // when the test runner did not set it.
        if std::env::var_os("ORQUESTRA_SEED_MCP_CATALOG").is_none() {
            let root = PathBuf::from("/tmp/orquestra-test-root");
            assert_eq!(seed_catalog_path(&root), root.join("seed-mcps.json"));
        }
    }

    mod deploy_cases;
    mod deploy_links;
}
