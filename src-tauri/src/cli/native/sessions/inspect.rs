//! Session inspect subcommand.

use super::is_session_active;
use super::super::{parse_options, parse_u32, rendered, CliError};
use crate::sessions::{get_session_transcript, get_unified_sessions};
use crate::sessions::types::{TranscriptTurn, UnifiedSession};

pub(super) fn inspect(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--session-id", "--harness", "--max-turns"],
        &["--compact", "--json"],
    )?;

    let positional_id = args.first().filter(|a| !a.starts_with('-')).cloned();
    let session_id_arg = options
        .one("--session-id")?
        .or(positional_id)
        .unwrap_or_else(|| "latest".to_string());

    let harness_opt = options.one("--harness")?;
    let max_turns = parse_u32(&options, "--max-turns")?.unwrap_or(10) as usize;
    let compact = options.has("--compact");

    let sessions = get_unified_sessions().map_err(CliError::Operation)?;

    let session = if session_id_arg == "latest" {
        sessions
            .into_iter()
            .find(|s| harness_opt.as_deref().map_or(true, |h| s.harness == h))
            .ok_or_else(|| CliError::Operation("no session found".to_string()))?
    } else {
        sessions
            .into_iter()
            .find(|s| {
                s.id == session_id_arg && harness_opt.as_deref().map_or(true, |h| s.harness == h)
            })
            .ok_or_else(|| CliError::Operation(format!("session '{session_id_arg}' not found")))?
    };

    let mut turns = get_session_transcript(session.harness.clone(), session.id.clone())
        .map_err(CliError::Operation)?;

    if turns.len() > max_turns {
        let skip = turns.len() - max_turns;
        turns.drain(..skip);
    }

    let active = is_session_active(&session.harness, &session.id);

    let report = serde_json::json!({
        "session": session,
        "isActive": active,
        "turns": turns,
    });

    let human = format_inspect_human(&session, active, &turns, compact);
    rendered(&report, options.has("--json"), human)
}

fn format_inspect_human(session: &UnifiedSession, active: bool, turns: &[TranscriptTurn], compact: bool) -> String {
    let mut out = format!(
        "Session:     {}\n\
         Harness:     {}\n\
         Title:       {}\n\
         Project:     {}\n\
         Updated:     {}\n\
         Active:      {}\n\
         Pinned:      {}\n\
         Turns Count: {}",
        session.id,
        session.harness,
        session.title,
        session.project_path.as_deref().unwrap_or("none"),
        session.updated_at,
        active,
        if session.is_pinned { "yes" } else { "no" },
        turns.len()
    );

    if turns.is_empty() {
        out.push_str("\n\n(Transcript empty or not parsed for this harness)");
        return out;
    }

    out.push_str("\n\nRecent Turns:\n");
    for (idx, turn) in turns.iter().enumerate() {
        let role = &turn.role;
        let timestamp = turn.timestamp.as_deref().unwrap_or("");
        if compact {
            let snippet = turn.content.lines().next().unwrap_or("");
            let clean = if snippet.len() > 60 { format!("{}...", &snippet[..57]) } else { snippet.to_string() };
            out.push_str(&format!("  [{:>2}] {:<10} {:<20} {}\n", idx + 1, role, timestamp, clean));
        } else {
            out.push_str(&format!("--- Turn {} ({}, {}) ---\n{}\n\n", idx + 1, role, timestamp, turn.content.trim()));
        }
    }

    out
}
