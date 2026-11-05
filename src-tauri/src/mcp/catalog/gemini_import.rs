//! Incremental adoption of MCPs already configured for managed harnesses.

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
    let previously_managed = catalog.managed_ids.clone();
    let mut changed = false;

    // Read only harness-owned user configs. Workspace `.agents` and
    // Freebuff account-level MCPs/skills are outside this filesystem catalog.
    for (rel, scope) in [
        (".agents/mcp.json", "freebuff"),
        (".gemini/config/mcp_config.json", "antigravity"),
        (".gemini/settings.json", "antigravity"),
    ] {
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
            if previously_managed.contains(id) {
                continue;
            }
            if !definition.is_object() {
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
            if crate::official::is_official_mcp(id)
                && url.is_none()
                && super::install::find_command(&command).is_none()
            {
                // Keep unavailable curated packages absent so the Store's
                // Add action installs them rather than importing broken paths.
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
            if let Some(existing) = catalog.mcps.get_mut(id) {
                if !existing
                    .enabled_harnesses
                    .iter()
                    .any(|enabled| enabled == scope)
                {
                    existing.enabled_harnesses.push(scope.to_string());
                    existing.enabled_harnesses.sort();
                    changed = true;
                }
                changed |= catalog.managed_ids.insert(id.clone());
                continue;
            }
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
                    enabled_harnesses: vec![scope.to_string()],
                    official: false,
                },
            );
            changed = true;
        }
    }
    if changed {
        save_catalog(paths, &catalog)?;
    }
    Ok(())
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
    fn adopts_global_mcp_servers_without_reenabling_removed_scopes() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("orquestra-gemini-mcp-{nonce}"));
        let home = base.join("home");
        let root = base.join("orquestra");
        fs::create_dir_all(home.join(".gemini/config")).expect("Gemini config");
        fs::create_dir_all(home.join(".agents")).expect("Freebuff config");
        let paths = OrquestraPaths {
            vault: root.join("skills-library"),
            active: root.join("active"),
            root,
        };
        paths.ensure_layout().expect("layout");
        fs::write(
            home.join(".agents/mcp.json"),
            r#"{"mcpServers":{"exa":{"url":"https://exa.example/mcp"},"rea":{"command":"/bin/echo","args":[]},"orquestra-idea-vault":{"command":"idea-vault","args":[]}}}"#,
        )
        .expect("Freebuff config");
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
        let mut catalog = load_catalog(&paths, &home.join("missing-seed.json")).expect("reload");
        assert_eq!(
            catalog.mcps.keys().cloned().collect::<Vec<_>>(),
            ["exa", "rea"]
        );
        assert_eq!(
            catalog.mcps["exa"].url.as_deref(),
            Some("https://exa.example/mcp")
        );
        assert_eq!(
            catalog.mcps["rea"].enabled_harnesses,
            ["antigravity", "freebuff"]
        );
        assert!(!catalog.mcps.contains_key("orquestra-idea-vault"));
        assert_eq!(
            catalog.managed_ids,
            ["exa", "rea"].into_iter().map(str::to_string).collect()
        );

        let rea = catalog.mcps.get_mut("rea").expect("rea adopted");
        rea.enabled_harnesses.retain(|scope| scope != "freebuff");
        save_catalog(&paths, &catalog).expect("save scope change");
        adopt_existing(&paths, &home, &home.join("missing-seed.json")).expect("second scan");
        let catalog = load_catalog(&paths, &home.join("missing-seed.json")).expect("reload");
        assert_eq!(catalog.mcps["rea"].enabled_harnesses, ["antigravity"]);
        let _ = fs::remove_dir_all(base);
    }
}
