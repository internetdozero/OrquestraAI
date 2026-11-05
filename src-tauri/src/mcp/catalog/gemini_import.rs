//! One-time adoption of MCPs already configured for Gemini.

use super::{load_catalog, save_catalog};
use crate::mcp::McpServer;
use crate::paths::OrquestraPaths;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub(super) fn adopt_existing(
    paths: &OrquestraPaths,
    home: &Path,
    seed: &Path,
) -> Result<(), String> {
    let mut catalog = load_catalog(paths, seed)?;
    if catalog.gemini_configs_imported {
        return Ok(());
    }

    // Gemini CLI and Antigravity have separate native configs, but share the
    // Antigravity scope in Orquestra's current harness model.
    for rel in [".gemini/config/mcp_config.json", ".gemini/settings.json"] {
        let path = home.join(rel);
        if has_symlink_ancestor(home, &path) || is_symlink(&path) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(root) = json5::from_str::<Value>(&content) else {
            continue;
        };
        let Some(servers) = root.get("mcpServers").and_then(Value::as_object) else {
            continue;
        };
        for (id, definition) in servers {
            if catalog.mcps.contains_key(id) || !definition.is_object() {
                continue;
            }
            let command = definition
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let url = ["url", "httpUrl"]
                .iter()
                .find_map(|key| definition.get(*key).and_then(Value::as_str))
                .map(str::to_string);
            if command.trim().is_empty() && url.is_none() {
                continue;
            }
            let args = definition
                .get("args")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let env = definition
                .get("env")
                .and_then(Value::as_object)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|value| (key.clone(), value.to_string()))
                        })
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();
            let headers = definition
                .get("headers")
                .and_then(Value::as_object)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|value| (key.clone(), value.to_string()))
                        })
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();
            catalog.managed_ids.insert(id.clone());
            catalog.mcps.insert(
                id.clone(),
                McpServer {
                    id: id.clone(),
                    name: id.clone(),
                    description: String::new(),
                    command,
                    args,
                    env,
                    url,
                    headers,
                    global_enabled: false,
                    enabled_harnesses: vec!["antigravity".to_string()],
                    official: false,
                },
            );
        }
    }
    catalog.gemini_configs_imported = true;
    save_catalog(paths, &catalog)
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
}

fn has_symlink_ancestor(home: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(home) else {
        return true;
    };
    let mut current = home.to_path_buf();
    let mut parts = relative.components().peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            break;
        }
        current.push(part);
        if is_symlink(&current) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn adopts_mcp_servers_from_gemini_configs_once() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("orquestra-gemini-mcp-{nonce}"));
        let home = base.join("home");
        let root = base.join("orquestra");
        fs::create_dir_all(home.join(".gemini/config")).expect("Gemini config");
        let paths = OrquestraPaths {
            vault: root.join("skills-library"),
            active: root.join("active"),
            root,
        };
        paths.ensure_layout().expect("layout");
        fs::write(
            home.join(".gemini/config/mcp_config.json"),
            r#"{"mcpServers":{"exa":{"url":"https://exa.example/mcp"}}}"#,
        )
        .expect("Antigravity config");
        fs::write(
            home.join(".gemini/settings.json"),
            r#"{"mcpServers":{"rea":{"command":"rea","args":["mcp"]}}}"#,
        )
        .expect("Gemini CLI config");

        adopt_existing(&paths, &home, &home.join("missing-seed.json")).expect("adopt");
        let catalog = load_catalog(&paths, &home.join("missing-seed.json")).expect("reload");
        assert_eq!(
            catalog.mcps.keys().cloned().collect::<Vec<_>>(),
            ["exa", "rea"]
        );
        assert_eq!(
            catalog.mcps["exa"].url.as_deref(),
            Some("https://exa.example/mcp")
        );
        assert_eq!(catalog.mcps["rea"].enabled_harnesses, ["antigravity"]);
        assert_eq!(
            catalog.managed_ids,
            ["exa", "rea"].into_iter().map(str::to_string).collect()
        );
        assert!(catalog.gemini_configs_imported);

        let _ = fs::remove_dir_all(base);
    }
}
