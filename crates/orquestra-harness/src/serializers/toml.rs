//! Codex-style TOML: `[mcp_servers.<id>]` tables, edited in place.

use std::collections::BTreeSet;

use super::json::sorted_env;
use crate::server::HarnessServer;

/// Codex reads `[mcp_servers.<id>]`. The audit notes said `[mcp.<id>]`, but the
/// live `~/.codex/config.toml` and `codex mcp list` both agree on
/// `mcp_servers`; an `[mcp]` table is unknown to Codex and would be ignored.
const CODEX_TABLE: &str = "mcp_servers";

/// Codex reads `[mcp_servers.<id>]` plus `[mcp_servers.<id>.env]`. The edit is
/// format-preserving, so comments, ordering and every unrelated table in
/// `config.toml` survive.
pub fn render_codex_toml(
    existing: &str,
    active: &[HarnessServer],
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
        let servers =
            codex_servers(&mut document).ok_or_else(|| format!("{CODEX_TABLE} is not a table"))?;

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
        if let Some(url) = &server.url {
            entry.insert("url", toml_edit::value(url.as_str()));
            entry.remove("command");
            entry.remove("args");
        } else {
            entry.remove("url");
            entry.insert("command", toml_edit::value(server.command.as_str()));

            let mut args = toml_edit::Array::new();
            for arg in &server.args {
                args.push(arg.as_str());
            }
            entry.insert("args", toml_edit::value(args));
        }

        if !server.headers.is_empty() {
            let headers = entry
                .entry("headers")
                .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            let Some(headers) = headers.as_table_mut() else {
                return Err(format!(
                    "[{CODEX_TABLE}.{}.headers] is not a table",
                    server.id
                ));
            };
            for (key, value) in &server.headers {
                headers.insert(key, toml_edit::value(value.as_str()));
            }
        }

        if !server.env.is_empty() {
            if entry
                .get("env")
                .map_or(true, |item| item.as_table().is_none())
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

fn codex_servers(document: &mut toml_edit::DocumentMut) -> Option<&mut toml_edit::Table> {
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
