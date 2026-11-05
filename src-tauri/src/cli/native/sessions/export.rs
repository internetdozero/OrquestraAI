//! Session transcript export subcommand.

use super::super::{parse_options, rendered, usage, CliError};
use crate::sessions::types::{TranscriptTurn, UnifiedSession};
use crate::sessions::{get_session_transcript, get_unified_sessions};
use std::fs;

pub(super) fn export(args: &[String]) -> Result<String, CliError> {
    let session_id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra sessions export <session-id> [--harness <id>] [--format <markdown|json>] [--output <path>] [--json]".to_string()))?;

    let options = parse_options(
        &args[1..],
        &["--harness", "--format", "--output"],
        &["--json"],
    )?;

    let harness_opt = options.one("--harness")?;
    let format = options
        .one("--format")?
        .unwrap_or_else(|| "markdown".to_string())
        .to_lowercase();
    let output_path = options.one("--output")?;

    let sessions = get_unified_sessions().map_err(CliError::Operation)?;

    let session: UnifiedSession = match session_id.as_str() {
        "latest" => sessions
            .into_iter()
            .find(|s| harness_opt.as_deref().map_or(true, |h| s.harness == h))
            .ok_or_else(|| CliError::Operation("no session found to export".to_string()))?,
        _ => sessions
            .into_iter()
            .find(|s| {
                s.id == *session_id && harness_opt.as_deref().map_or(true, |h| s.harness == h)
            })
            .ok_or_else(|| CliError::Operation(format!("session '{session_id}' not found")))?,
    };

    let turns = get_session_transcript(session.harness.clone(), session.id.clone())
        .map_err(CliError::Operation)?;

    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&serde_json::json!({
            "session": session,
            "turns": turns,
        }))
        .map_err(|e| CliError::Operation(format!("serialization failed: {e}")))?,
        _ => render_markdown(&session, &turns),
    };

    if let Some(dest) = output_path {
        fs::write(&dest, &content)
            .map_err(|e| CliError::Operation(format!("failed to write export to '{dest}': {e}")))?;

        let report = serde_json::json!({
            "sessionId": session.id,
            "harness": session.harness,
            "format": format,
            "outputPath": dest,
            "bytes": content.len(),
        });
        let human = format!("Exported session '{}' ({format}) to '{dest}'.", session.id);
        return rendered(&report, options.has("--json"), human);
    }

    rendered(&content, options.has("--json"), content.clone())
}

fn render_markdown(session: &UnifiedSession, turns: &[TranscriptTurn]) -> String {
    let mut out = format!(
        "# Session Export: {}\n\n\
         - **ID:** `{}`\n\
         - **Harness:** `{}`\n\
         - **Project:** `{}`\n\
         - **Updated:** `{}`\n\n\
         ## Transcript ({} turns)\n\n",
        session.title,
        session.id,
        session.harness,
        session.project_path.as_deref().unwrap_or("none"),
        session.updated_at,
        turns.len()
    );

    for (i, turn) in turns.iter().enumerate() {
        let role = &turn.role;
        let ts = turn.timestamp.as_deref().unwrap_or("");
        out.push_str(&format!(
            "### Turn {} — {} ({})\n\n{}\n\n",
            i + 1,
            role,
            ts,
            turn.content.trim()
        ));
    }

    out
}
