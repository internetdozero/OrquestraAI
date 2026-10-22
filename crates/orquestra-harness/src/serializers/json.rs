//! JSON and JSONC envelopes: `mcpServers` (Claude, Antigravity, Copilot) and
//! the `mcp` block with a merged command array (OpenCode, Kilo Code).

use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::BTreeSet;

use crate::server::HarnessServer;

/// Keys sorted so the same catalog always renders byte-identically.
pub(super) fn sorted_env(server: &HarnessServer) -> Vec<(&str, &str)> {
    let mut entries: Vec<(&str, &str)> = server
        .env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    entries
}

fn json_args(server: &HarnessServer) -> JsonValue {
    JsonValue::Array(server.args.iter().cloned().map(JsonValue::String).collect())
}

/// Env values win over the file's own, but keys OrquestraAI does not define are
/// left alone so hand-added variables never disappear.
fn merged_env(existing: Option<&JsonValue>, server: &HarnessServer) -> JsonValue {
    let mut object = match existing {
        Some(JsonValue::Object(map)) => map.clone(),
        _ => JsonMap::new(),
    };

    for (key, value) in sorted_env(server) {
        object.insert(key.to_string(), JsonValue::String(value.to_string()));
    }

    JsonValue::Object(object)
}

/// Overwrite only the keys OrquestraAI owns on one entry. A `null` field means
/// "this key must not be here", which is how the merged-array shapes drop a
/// stale `args` key.
fn merge_entry(
    existing: Option<&JsonValue>,
    env_key: &str,
    fields: &[(&str, JsonValue)],
    server: &HarnessServer,
) -> JsonValue {
    let mut object = match existing {
        Some(JsonValue::Object(map)) => map.clone(),
        _ => JsonMap::new(),
    };

    for (key, value) in fields {
        if value.is_null() {
            object.remove(*key);
        } else {
            object.insert((*key).to_string(), value.clone());
        }
    }

    if !server.env.is_empty() {
        let merged = merged_env(object.get(env_key), server);
        object.insert(env_key.to_string(), merged);
    }

    if !server.headers.is_empty() {
        let mut headers = match object.get("headers") {
            Some(JsonValue::Object(map)) => map.clone(),
            _ => JsonMap::new(),
        };
        for (key, value) in &server.headers {
            headers.insert(key.clone(), JsonValue::String(value.clone()));
        }
        object.insert("headers".to_string(), JsonValue::Object(headers));
    }

    JsonValue::Object(object)
}

fn parse_json_document(existing: &str) -> Result<JsonValue, String> {
    if existing.trim().is_empty() {
        return Ok(JsonValue::Object(JsonMap::new()));
    }

    json5::from_str(existing).map_err(|error| format!("config is not valid JSONC: {error}"))
}

/// Canonical form of a JSON document: two-space indent and one trailing
/// newline, so a target already in this form is left byte-identical.
pub(super) fn render_pretty(value: &JsonValue) -> Result<String, String> {
    let mut rendered = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize config: {error}"))?;
    rendered.push('\n');
    Ok(rendered)
}

/// Antigravity, Copilot and the root of `~/.claude.json` all take
/// `{"mcpServers": { … }}`; for Claude the surrounding file is Claude's own
/// state, which is preserved because only that one key is touched.
pub fn render_mcp_servers(
    existing: &str,
    active: &[HarnessServer],
    stale: &BTreeSet<String>,
    entry_type: Option<&str>,
) -> Result<String, String> {
    let mut root = parse_json_document(existing)?;
    let object = root
        .as_object_mut()
        .ok_or_else(|| "config root is not an object".to_string())?;

    if let Some(value) = object.get("mcpServers") {
        if !value.is_object() {
            return Err("mcpServers is not an object".to_string());
        }
    }

    if let Some(servers) = object
        .get_mut("mcpServers")
        .and_then(JsonValue::as_object_mut)
    {
        for id in stale {
            servers.remove(id);
        }
    }

    if !active.is_empty() {
        if !object.contains_key("mcpServers") {
            object.insert("mcpServers".to_string(), JsonValue::Object(JsonMap::new()));
        }

        let servers = object
            .get_mut("mcpServers")
            .and_then(JsonValue::as_object_mut)
            .ok_or_else(|| "mcpServers is not an object".to_string())?;

        for server in active {
            let mut fields: Vec<(&str, JsonValue)> = Vec::new();
            if let Some(kind) = entry_type {
                let kind = if server.is_remote() { "http" } else { kind };
                fields.push(("type", JsonValue::String(kind.to_string())));
            }
            if let Some(url) = &server.url {
                fields.push(("url", JsonValue::String(url.clone())));
                fields.push(("command", JsonValue::Null));
                fields.push(("args", JsonValue::Null));
            } else {
                fields.push(("url", JsonValue::Null));
                fields.push(("command", JsonValue::String(server.command.clone())));
                fields.push(("args", json_args(server)));
            }
            fields.push(("env", JsonValue::Null));

            let merged = merge_entry(servers.get(&server.id), "env", &fields, server);
            servers.insert(server.id.clone(), merged);
        }
    }

    render_pretty(&root)
}

/// OpenCode and Kilo Code want `{"mcp": …}` with a merged command array, an
/// `environment` key, `type: "local"` and `enabled: true`.
pub fn render_jsonc_block(
    existing: &str,
    active: &[HarnessServer],
    stale: &BTreeSet<String>,
) -> Result<String, String> {
    let mut root = parse_json_document(existing)?;
    let object = root
        .as_object_mut()
        .ok_or_else(|| "config root is not an object".to_string())?;

    if let Some(value) = object.get("mcp") {
        if !value.is_object() {
            return Err("mcp is not an object".to_string());
        }
    }

    if let Some(servers) = object.get_mut("mcp").and_then(JsonValue::as_object_mut) {
        for id in stale {
            servers.remove(id);
        }
    }

    if !active.is_empty() {
        if !object.contains_key("mcp") {
            object.insert("mcp".to_string(), JsonValue::Object(JsonMap::new()));
        }

        let servers = object
            .get_mut("mcp")
            .and_then(JsonValue::as_object_mut)
            .ok_or_else(|| "mcp is not an object".to_string())?;

        for server in active {
            let mut command = Vec::with_capacity(server.args.len() + 1);
            command.push(JsonValue::String(server.command.clone()));
            command.extend(server.args.iter().cloned().map(JsonValue::String));

            let mut fields: Vec<(&str, JsonValue)> = vec![
                (
                    "type",
                    JsonValue::String(
                        if server.is_remote() {
                            "remote"
                        } else {
                            "local"
                        }
                        .to_string(),
                    ),
                ),
                ("enabled", JsonValue::Bool(true)),
                ("environment", JsonValue::Null),
                ("args", JsonValue::Null),
            ];
            if let Some(url) = &server.url {
                fields.push(("url", JsonValue::String(url.clone())));
                fields.push(("command", JsonValue::Null));
            } else {
                fields.push(("url", JsonValue::Null));
                fields.push(("command", JsonValue::Array(command)));
            }

            let merged = merge_entry(servers.get(&server.id), "environment", &fields, server);
            servers.insert(server.id.clone(), merged);
        }
    }

    render_pretty(&root)
}
