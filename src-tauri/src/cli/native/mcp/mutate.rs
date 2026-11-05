//! Mutation subcommands: add, remove, enable, disable, set-state.

use std::collections::HashMap;
use super::super::{parse_options, rendered, usage, CliError};
use crate::mcp::{delete_mcp, save_mcp, set_mcp_state, McpServer};

pub(super) fn add(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &[
            "--id",
            "--name",
            "--command",
            "--args",
            "--arg",
            "--env",
            "--url",
            "--header",
            "--description",
            "--harness",
        ],
        &["--global", "--json"],
    )?;

    let id = options.required("--id")?;
    let name = options.required("--name")?;
    let command = options.required("--command")?;

    let mut server_args = Vec::new();
    if let Some(csv_args) = options.one("--args")? {
        server_args.extend(csv_args.split(',').map(|s| s.trim().to_string()));
    }
    server_args.extend(options.repeated("--arg"));

    let mut env = HashMap::new();
    for entry in options.repeated("--env") {
        if let Some((k, v)) = entry.split_once('=') {
            env.insert(k.trim().to_string(), v.to_string());
        } else {
            return Err(usage(format!("invalid --env format '{entry}', expected KEY=VAL")));
        }
    }

    let mut headers = HashMap::new();
    for entry in options.repeated("--header") {
        if let Some((k, v)) = entry.split_once('=') {
            headers.insert(k.trim().to_string(), v.to_string());
        } else if let Some((k, v)) = entry.split_once(':') {
            headers.insert(k.trim().to_string(), v.trim().to_string());
        } else {
            return Err(usage(format!("invalid --header format '{entry}', expected KEY=VAL or KEY:VAL")));
        }
    }

    let description = options.one("--description")?.unwrap_or_default();
    let url = options.one("--url")?;
    let global_enabled = options.has("--global");
    let enabled_harnesses = options.repeated("--harness");

    let server = McpServer {
        id: id.clone(),
        name,
        description,
        command,
        args: server_args,
        env,
        url,
        headers,
        global_enabled,
        enabled_harnesses,
        official: false,
    };

    save_mcp(server.clone()).map_err(CliError::Operation)?;

    let human = format!("Added MCP server '{id}' to catalog and projected into harnesses.");
    rendered(&server, options.has("--json"), human)
}

pub(super) fn remove(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra mcp remove <id> [--json]".to_string()))?;

    let options = parse_options(&args[1..], &[], &["--json"])?;

    delete_mcp(id).map_err(CliError::Operation)?;

    let confirmation = serde_json::json!({
        "deleted": true,
        "id": id,
    });
    let human = format!("Removed MCP server '{id}' from catalog and purged from harness configurations.");
    rendered(&confirmation, options.has("--json"), human)
}

pub(super) fn enable(args: &[String]) -> Result<String, CliError> {
    toggle(args, true, "enable")
}

pub(super) fn disable(args: &[String]) -> Result<String, CliError> {
    toggle(args, false, "disable")
}

pub(super) fn set_state(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra mcp set-state <id> <on|off> [--global] [--harness <name>] [--json]".to_string()))?;

    let state_arg = args
        .get(1)
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra mcp set-state <id> <on|off> [--global] [--harness <name>] [--json]".to_string()))?;

    let enabled = match state_arg.as_str() {
        "on" | "true" | "enable" => true,
        "off" | "false" | "disable" => false,
        other => return Err(usage(format!("expected on|off, got '{other}'"))),
    };

    apply_toggle(id, &args[2..], enabled)
}

fn toggle(args: &[String], enabled: bool, verb: &str) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage(format!("usage: orquestra mcp {verb} <id> [--global] [--harness <name>] [--json]")))?;

    apply_toggle(id, &args[1..], enabled)
}

fn apply_toggle(id: &str, tail_args: &[String], enabled: bool) -> Result<String, CliError> {
    let options = parse_options(tail_args, &["--harness"], &["--global", "--json"])?;

    let target_harness = options.one("--harness")?;
    let scope_label = if let Some(harness) = &target_harness {
        harness.as_str()
    } else {
        "global"
    };

    set_mcp_state(id, target_harness.as_deref(), enabled).map_err(CliError::Operation)?;

    let state_label = if enabled { "enabled" } else { "disabled" };
    let human = format!("MCP server '{id}' {state_label} for {scope_label} scope.");
    let report = serde_json::json!({
        "id": id,
        "scope": scope_label,
        "enabled": enabled,
    });

    rendered(&report, options.has("--json"), human)
}
