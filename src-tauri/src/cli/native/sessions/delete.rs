//! Surgical session deletion subcommand.

use super::super::{parse_options, rendered, usage, CliError};
use super::is_session_active;
use crate::sessions::types::PurgeFilter;
use crate::sessions::{get_unified_sessions, purge_sessions, toggle_pin_session};

pub(super) fn delete(args: &[String]) -> Result<String, CliError> {
    let session_id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: ownhub sessions delete <session-id> --harness <id> [--yes] [--force] [--json]".to_string()))?;

    let options = parse_options(
        &args[1..],
        &["--harness"],
        &["--yes", "-y", "--force", "--json"],
    )?;

    let harness = options.required("--harness")?;
    let _confirmed = options.has("--yes") || options.has("-y");
    let force = options.has("--force");

    let sessions = get_unified_sessions().map_err(CliError::Operation)?;
    let target = sessions
        .into_iter()
        .find(|s| s.id == *session_id && s.harness == harness)
        .ok_or_else(|| CliError::Operation(format!("session not found in harness '{harness}'")))?;

    if is_session_active(&target.harness, &target.id) {
        return Err(CliError::Operation(
            "refusing to delete an active session currently held open by running agent".to_string(),
        ));
    }

    if target.is_pinned {
        if !force {
            return Err(CliError::Operation(
                "refusing to delete a pinned session; unpin first or specify --force".to_string(),
            ));
        }
        // Force unpin before deletion
        let _ = toggle_pin_session(harness.clone(), session_id.to_string(), None);
    }

    let filter = PurgeFilter {
        harness: Some(harness.clone()),
        session_ids: Some(vec![session_id.to_string()]),
        ignore_pinned: false,
        ..PurgeFilter::default()
    };

    let report = purge_sessions(filter).map_err(CliError::Operation)?;

    let result = serde_json::json!({
        "sessionId": session_id,
        "harness": harness,
        "deleted": report.deleted_count > 0,
        "reclaimedBytes": report.reclaimed_bytes,
    });

    let human = format!(
        "Deleted session [{harness}], reclaimed {} bytes.",
        report.reclaimed_bytes
    );

    rendered(&result, options.has("--json"), human)
}
