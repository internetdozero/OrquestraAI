//! Projection and harness target subcommands.

use super::super::{parse_options, rendered, CliError};
use crate::mcp::{deploy_mcps_reporting, list_targets};

pub(super) fn deploy(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--json"])?;
    let changed = deploy_mcps_reporting().map_err(CliError::Operation)?;
    let targets = list_targets().map_err(CliError::Operation)?;
    let active_targets = targets.iter().filter(|t| t.installed).count();

    let report = serde_json::json!({
        "changed": changed,
        "totalSyncedHarnesses": active_targets,
    });

    let mut lines = Vec::new();
    lines.push("MCP deployment completed:".to_string());
    if changed.is_empty() {
        lines.push("  All harness configuration files were already up-to-date.".to_string());
    } else {
        for path in &changed {
            lines.push(format!("  Updated: {path}"));
        }
    }
    lines.push(format!("  Total synced harnesses: {active_targets}"));

    rendered(&report, options.has("--json"), lines.join("\n"))
}

pub(super) fn targets(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--json"])?;
    let targets = list_targets().map_err(CliError::Operation)?;

    let mut lines = Vec::with_capacity(targets.len() + 1);
    lines.push(format!(
        "{:<12} {:<15} {}",
        "TARGET", "STATUS", "CONFIG PATH"
    ));
    for target in &targets {
        let status = if target.installed {
            "Installed"
        } else {
            "Not Installed"
        };
        lines.push(format!(
            "{:<12} {:<15} {}",
            target.id, status, target.config_path
        ));
    }

    rendered(&targets, options.has("--json"), lines.join("\n"))
}
