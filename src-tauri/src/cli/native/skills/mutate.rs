//! Skill mutation subcommands: enable, disable, batch, and delete.

use super::super::{parse_options, rendered, usage, CliError};
use crate::skills::{batch_set_all, delete_skill, list_skills, set_skill_state};

pub(super) fn enable(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills enable <id> [--scope <scope>] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--scope"], &["--json"])?;
    let scope = options.one("--scope")?.unwrap_or_else(|| "global".to_string());

    set_skill_state(id, &scope, true).map_err(CliError::Operation)?;

    let report = serde_json::json!({
        "id": id,
        "scope": scope,
        "enabled": true,
    });
    let human = format!("Skill '{id}' enabled for scope '{scope}'.");
    rendered(&report, options.has("--json"), human)
}

pub(super) fn disable(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills disable <id> [--scope <scope>] [--force] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--scope"], &["--force", "--json"])?;
    let scope = options.one("--scope")?.unwrap_or_else(|| "global".to_string());
    let force = options.has("--force");

    if scope != "global" && !force {
        let skills = list_skills(Some(scope.clone())).map_err(CliError::Operation)?;
        if let Some(item) = skills.into_iter().find(|s| s.id == *id) {
            if item.inherited_from_global {
                return Err(CliError::Operation(format!(
                    "Skill '{id}' is inherited from Global; disable it in Global first or use --scope global"
                )));
            }
        }
    }

    set_skill_state(id, &scope, false).map_err(CliError::Operation)?;

    let report = serde_json::json!({
        "id": id,
        "scope": scope,
        "enabled": false,
    });
    let human = format!("Skill '{id}' disabled for scope '{scope}'.");
    rendered(&report, options.has("--json"), human)
}

pub(super) fn batch(args: &[String]) -> Result<String, CliError> {
    let action = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills batch <enable|disable> [--scope <scope>] [--json]".to_string()))?;

    let enabled = match action.as_str() {
        "enable" | "on" => true,
        "disable" | "off" => false,
        other => return Err(usage(format!("expected 'enable' or 'disable', got '{other}'"))),
    };

    let options = parse_options(&args[1..], &["--scope"], &["--json"])?;
    let scope = options.one("--scope")?.unwrap_or_else(|| "global".to_string());

    batch_set_all(scope.clone(), enabled).map_err(CliError::Operation)?;

    let action_label = if enabled { "enabled" } else { "disabled" };
    let report = serde_json::json!({
        "scope": scope,
        "action": action_label,
        "success": true,
    });
    let human = format!("Batch {action_label} all skills for scope '{scope}'.");
    rendered(&report, options.has("--json"), human)
}

pub(super) fn delete(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills delete <id> [--force] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &[], &["--force", "-f", "--json"])?;
    delete_skill(id.to_string()).map_err(CliError::Operation)?;

    let report = serde_json::json!({
        "id": id,
        "deleted": true,
    });
    let human = format!("Permanently deleted skill '{id}' from vault and unlinked projections.");
    rendered(&report, options.has("--json"), human)
}
