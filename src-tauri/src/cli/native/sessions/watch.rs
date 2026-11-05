//! Tail and watch subcommands.

use super::super::{parse_options, parse_u32, rendered, CliError};
use super::is_session_active;
use crate::sessions::types::{TranscriptTurn, UnifiedSession};
use crate::sessions::{get_session_transcript, get_unified_sessions};

pub(super) fn tail(args: &[String]) -> Result<String, CliError> {
    run_tail_watch(args, false)
}

pub(super) fn watch(args: &[String]) -> Result<String, CliError> {
    run_tail_watch(args, true)
}

fn run_tail_watch(args: &[String], default_follow: bool) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--harness", "--limit"],
        &["--follow", "-f", "--json"],
    )?;

    let positional_id = args.first().filter(|a| !a.starts_with('-')).cloned();
    let harness_opt = options.one("--harness")?;
    let limit = parse_u32(&options, "--limit")?.unwrap_or(5) as usize;
    let _follow = default_follow || options.has("--follow") || options.has("-f");

    let sessions = get_unified_sessions().map_err(CliError::Operation)?;

    let target_session: UnifiedSession = match positional_id {
        Some(id) => sessions
            .into_iter()
            .find(|s| s.id == id && harness_opt.as_deref().map_or(true, |h| s.harness == h))
            .ok_or_else(|| CliError::Operation(format!("session '{id}' not found")))?,
        None => {
            // Find newest active session or newest session
            sessions
                .iter()
                .find(|s| {
                    is_session_active(&s.harness, &s.id)
                        && harness_opt.as_deref().map_or(true, |h| s.harness == h)
                })
                .or_else(|| {
                    sessions
                        .iter()
                        .find(|s| harness_opt.as_deref().map_or(true, |h| s.harness == h))
                })
                .cloned()
                .ok_or_else(|| CliError::Operation("no session found to tail/watch".to_string()))?
        }
    };

    let mut turns =
        get_session_transcript(target_session.harness.clone(), target_session.id.clone())
            .map_err(CliError::Operation)?;

    if turns.len() > limit {
        let skip = turns.len() - limit;
        turns.drain(..skip);
    }

    let report = serde_json::json!({
        "sessionId": target_session.id,
        "harness": target_session.harness,
        "turns": turns,
    });

    let human = format_tail_human(&target_session, &turns);
    rendered(&report, options.has("--json"), human)
}

fn format_tail_human(session: &UnifiedSession, turns: &[TranscriptTurn]) -> String {
    let mut out = format!(
        "Tailing [{}] {} ({}):\n",
        session.harness, session.id, session.title
    );
    if turns.is_empty() {
        out.push_str("  (No transcript turns available)");
        return out;
    }
    for turn in turns {
        let role = &turn.role;
        let ts = turn.timestamp.as_deref().unwrap_or("");
        let snippet = turn.content.trim();
        out.push_str(&format!("[{ts}] {role}:\n{snippet}\n\n"));
    }
    out
}
