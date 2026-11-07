//! Session pin and unpin subcommands.

use super::super::{parse_options, rendered, usage, CliError};
use crate::sessions::{get_unified_sessions, toggle_pin_session};

pub(super) fn pin(args: &[String]) -> Result<String, CliError> {
    let session_id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra sessions pin <session-id> [--harness <id>] [--label <text>] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--harness", "--label"], &["--json"])?;
    let harness = resolve_harness(session_id, options.one("--harness")?)?;
    let label = options.one("--label")?;

    let is_pinned = toggle_pin_session(harness.clone(), session_id.to_string(), label.clone())
        .map_err(CliError::Operation)?;

    // If it was already pinned and toggle unpinned it, re-toggle to pin
    if !is_pinned {
        toggle_pin_session(harness.clone(), session_id.to_string(), label.clone())
            .map_err(CliError::Operation)?;
    }

    let report = serde_json::json!({
        "harness": harness,
        "sessionId": session_id,
        "isPinned": true,
        "label": label,
    });
    let human = format!("Pinned session [{harness}].");
    rendered(&report, options.has("--json"), human)
}

pub(super) fn unpin(args: &[String]) -> Result<String, CliError> {
    let session_id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| {
            usage(
                "usage: orquestra sessions unpin <session-id> [--harness <id>] [--json]"
                    .to_string(),
            )
        })?;

    let options = parse_options(&args[1..], &["--harness"], &["--json"])?;
    let harness = resolve_harness(session_id, options.one("--harness")?)?;

    let is_pinned = toggle_pin_session(harness.clone(), session_id.to_string(), None)
        .map_err(CliError::Operation)?;

    // If it was not pinned and toggle just pinned it, re-toggle to unpin
    if is_pinned {
        toggle_pin_session(harness.clone(), session_id.to_string(), None)
            .map_err(CliError::Operation)?;
    }

    let report = serde_json::json!({
        "harness": harness,
        "sessionId": session_id,
        "isPinned": false,
    });
    let human = format!("Unpinned session [{harness}].");
    rendered(&report, options.has("--json"), human)
}

fn resolve_harness(session_id: &str, explicit: Option<String>) -> Result<String, CliError> {
    if let Some(h) = explicit {
        return Ok(h);
    }
    let sessions = get_unified_sessions().map_err(CliError::Operation)?;
    let matches: Vec<_> = sessions
        .into_iter()
        .filter(|s| s.id == session_id)
        .collect();
    match matches.len() {
        1 => Ok(matches[0].harness.clone()),
        0 => Err(CliError::Operation(
            "session not found; specify --harness".to_string(),
        )),
        _ => Err(usage(
            "ambiguous session found in multiple harnesses; specify --harness".to_string(),
        )),
    }
}
