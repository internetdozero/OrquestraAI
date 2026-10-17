//! Adapter serializers: one server definition, one dialect per harness.
//!
//! JSON targets are edited in place so foreign keys and hand-written
//! entries survive; Codex goes through `toml_edit`, which keeps its
//! comments, key order and unrelated tables byte-identical.

use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::BTreeSet;

use super::McpServer;

/// Codex reads `[mcp_servers.<id>]`. The audit notes said `[mcp.<id>]`, but the
/// live `~/.codex/config.toml` and `codex mcp list` both agree on
/// `mcp_servers`; an `[mcp]` table is unknown to Codex and would be ignored.
const CODEX_TABLE: &str = "mcp_servers";

/// Keys sorted so the same catalog always renders byte-identically.
pub(super) fn sorted_env(server: &McpServer) -> Vec<(&str, &str)> {
    let mut entries: Vec<(&str, &str)> = server
        .env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    entries
}

pub(super) fn json_args(server: &McpServer) -> JsonValue {
    JsonValue::Array(
        server
            .args
            .iter()
            .cloned()
            .map(JsonValue::String)
            .collect(),
    )
}

/// Env values win over the file's own, but keys OrquestraAI does not define are
/// left alone so hand-added variables never disappear.
pub(super) fn merged_env(existing: Option<&JsonValue>, server: &McpServer) -> JsonValue {
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
pub(super) fn merge_entry(
    existing: Option<&JsonValue>,
    env_key: &str,
    fields: &[(&str, JsonValue)],
    server: &McpServer,
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

    JsonValue::Object(object)
}

pub(super) fn parse_json_document(existing: &str) -> Result<JsonValue, String> {
    if existing.trim().is_empty() {
        return Ok(JsonValue::Object(JsonMap::new()));
    }

    json5::from_str(existing).map_err(|error| format!("config is not valid JSONC: {error}"))
}

pub(super) fn render_pretty(value: &JsonValue) -> Result<String, String> {
    let mut rendered = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize config: {error}"))?;
    rendered.push('\n');
    Ok(rendered)
}

/// Antigravity, Copilot and the root of `~/.claude.json` all take
/// `{"mcpServers": { … }}`; for Claude the surrounding file is Claude's own
/// state, which is preserved because only that one key is touched.
pub(super) fn render_mcp_servers(
    existing: &str,
    active: &[McpServer],
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
                fields.push(("type", JsonValue::String(kind.to_string())));
            }
            fields.push(("command", JsonValue::String(server.command.clone())));
            fields.push(("args", json_args(server)));
            fields.push(("env", JsonValue::Null));

            let merged = merge_entry(servers.get(&server.id), "env", &fields, server);
            servers.insert(server.id.clone(), merged);
        }
    }

    render_pretty(&root)
}

/// OpenCode and Kilo Code want `{"mcp": …}` with a merged command array, an
/// `environment` key, `type: "local"` and `enabled: true`.
pub(super) fn render_jsonc_block(
    existing: &str,
    active: &[McpServer],
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

            let fields: Vec<(&str, JsonValue)> = vec![
                ("type", JsonValue::String("local".to_string())),
                ("command", JsonValue::Array(command)),
                ("enabled", JsonValue::Bool(true)),
                ("environment", JsonValue::Null),
                ("args", JsonValue::Null),
            ];

            let merged = merge_entry(servers.get(&server.id), "environment", &fields, server);
            servers.insert(server.id.clone(), merged);
        }
    }

    render_pretty(&root)
}

/// Codex reads `[mcp_servers.<id>]` plus `[mcp_servers.<id>.env]`. The edit is
/// format-preserving, so comments, ordering and every unrelated table in
/// `config.toml` survive.
pub(super) fn render_codex_toml(
    existing: &str,
    active: &[McpServer],
    stale: &BTreeSet<String>,
) -> Result<String, String> {
    let mut document: toml_edit::DocumentMut = if existing.trim().is_empty() {
        toml_edit::DocumentMut::new()
    } else {
        existing
            .parse()
            .map_err(|error| format!("config is not valid TOML: {error}"))?
    };

    for id in stale {
        if let Some(servers) = codex_servers(&mut document) {
            servers.remove(id);
        }
    }

    for server in active {
        let servers = codex_servers(&mut document)
            .ok_or_else(|| format!("{CODEX_TABLE} is not a table"))?;

        if servers
            .get(&server.id)
            .is_some_and(|item| item.as_table().is_none())
        {
            return Err(format!("[{CODEX_TABLE}.{}] is not a table", server.id));
        }

        if !servers.contains_key(&server.id) {
            servers.insert(&server.id, toml_edit::Item::Table(toml_edit::Table::new()));
        }

        let entry = servers
            .get_mut(&server.id)
            .and_then(|item| item.as_table_mut())
            .ok_or_else(|| format!("[{CODEX_TABLE}.{}] is not a table", server.id))?;

        // Only the keys OrquestraAI owns are set, so `startup_timeout_sec` and
        // other hand-tuned fields stay on the entry.
        entry.insert("command", toml_edit::value(server.command.as_str()));

        let mut args = toml_edit::Array::new();
        for arg in &server.args {
            args.push(arg.as_str());
        }
        entry.insert("args", toml_edit::value(args));

        if !server.env.is_empty() {
            if entry
                .get("env")
                .is_none_or(|item| item.as_table().is_none())
            {
                entry.insert("env", toml_edit::Item::Table(toml_edit::Table::new()));
            }

            if let Some(env) = entry.get_mut("env").and_then(|item| item.as_table_mut()) {
                for (key, value) in sorted_env(server) {
                    env.insert(key, toml_edit::value(value));
                }
            }
        }
    }

    // Do not leave an empty table behind when nothing is managed any more.
    let empty = document
        .as_table()
        .get(CODEX_TABLE)
        .and_then(|item| item.as_table())
        .is_some_and(toml_edit::Table::is_empty);
    if empty {
        document.as_table_mut().remove(CODEX_TABLE);
    }

    Ok(document.to_string())
}

pub(super) fn codex_servers(document: &mut toml_edit::DocumentMut) -> Option<&mut toml_edit::Table> {
    let root = document.as_table_mut();

    if !root.contains_key(CODEX_TABLE) {
        let mut table = toml_edit::Table::new();
        // Keep the file's style: `mcp_servers` is only ever an implicit parent
        // of the per-server tables, never a printed header of its own.
        table.set_implicit(true);
        root.insert(CODEX_TABLE, toml_edit::Item::Table(table));
    }

    root.get_mut(CODEX_TABLE)
        .and_then(|item| item.as_table_mut())
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------
