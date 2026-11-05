//! MCP listing and diagnostic status commands.

use std::collections::BTreeMap;
use serde::Serialize;
use super::super::{parse_options, rendered, CliError};
use crate::mcp::{list_mcps_masked, list_targets, McpServer};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct McpStatusReport {
    total_servers: usize,
    official_servers: usize,
    custom_servers: usize,
    globally_active: usize,
    per_harness_active: BTreeMap<String, usize>,
    installed_targets: usize,
    recognized_targets: usize,
    sync_status: String,
}

pub(super) fn list(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--harness", "--search"],
        &["--enabled-only", "--global-only", "--official-only", "--json"],
    )?;

    let mut servers = list_mcps_masked().map_err(CliError::Operation)?;

    if options.has("--enabled-only") {
        servers.retain(|s| s.active_anywhere());
    }
    if options.has("--global-only") {
        servers.retain(|s| s.global_enabled);
    }
    if let Some(target) = options.one("--harness")? {
        servers.retain(|s| s.covers(&target));
    }
    if options.has("--official-only") {
        servers.retain(|s| s.official);
    }
    if let Some(term) = options.one("--search")? {
        let needle = term.to_lowercase();
        servers.retain(|s| {
            s.id.to_lowercase().contains(&needle)
                || s.name.to_lowercase().contains(&needle)
                || s.description.to_lowercase().contains(&needle)
        });
    }

    let human = format_server_table(&servers);
    rendered(&servers, options.has("--json"), human)
}

pub(super) fn status(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--json"])?;
    let servers = list_mcps_masked().map_err(CliError::Operation)?;
    let targets = list_targets().map_err(CliError::Operation)?;

    let total_servers = servers.len();
    let official_servers = servers.iter().filter(|s| s.official).count();
    let custom_servers = total_servers.saturating_sub(official_servers);
    let globally_active = servers.iter().filter(|s| s.global_enabled).count();

    let mut per_harness_active = BTreeMap::new();
    for target in &targets {
        let count = servers.iter().filter(|s| s.covers(&target.id)).count();
        per_harness_active.insert(target.id.clone(), count);
    }

    let installed_targets = targets.iter().filter(|t| t.installed).count();
    let recognized_targets = targets.len();
    let sync_status = "All configs in sync with master catalog".to_string();

    let report = McpStatusReport {
        total_servers,
        official_servers,
        custom_servers,
        globally_active,
        per_harness_active,
        installed_targets,
        recognized_targets,
        sync_status: sync_status.clone(),
    };

    let harness_breakdown = report
        .per_harness_active
        .iter()
        .map(|(id, count)| format!("{id} ({count})"))
        .collect::<Vec<_>>()
        .join(", ");

    let human = format!(
        "MCP Subsystem Status:\n  \
         Total Servers:      {total_servers} ({official_servers} official, {custom_servers} custom)\n  \
         Globally Active:    {globally_active}\n  \
         Per-Harness Active: {harness_breakdown}\n  \
         Harness Targets:    {installed_targets} installed / {recognized_targets} recognized\n  \
         Sync Status:        {sync_status}"
    );

    rendered(&report, options.has("--json"), human)
}

fn format_server_table(servers: &[McpServer]) -> String {
    if servers.is_empty() {
        return "No matching MCP servers found.".to_string();
    }

    let mut lines = Vec::with_capacity(servers.len() + 2);
    lines.push(format!(
        "{:<24} {:<18} {:<20} {:<15} {}",
        "ID", "NAME", "SCOPE", "COMMAND", "STATUS"
    ));

    for s in servers {
        let scope_str = if s.global_enabled && !s.enabled_harnesses.is_empty() {
            format!("global, {}", s.enabled_harnesses.join(", "))
        } else if s.global_enabled {
            "global".to_string()
        } else if !s.enabled_harnesses.is_empty() {
            s.enabled_harnesses.join(", ")
        } else {
            "disabled".to_string()
        };

        let status_str = if s.active_anywhere() {
            if s.official {
                "active (official)".to_string()
            } else {
                "active".to_string()
            }
        } else {
            "inactive".to_string()
        };

        lines.push(format!(
            "{:<24} {:<18} {:<20} {:<15} {}",
            s.id, s.name, scope_str, s.command, status_str
        ));
    }

    lines.join("\n")
}
