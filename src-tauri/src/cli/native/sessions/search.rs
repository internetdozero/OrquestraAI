//! Session history search subcommand.

use super::super::{parse_options, parse_u32, rendered, usage, CliError};
use super::is_session_active;
use super::list::format_session;
use crate::sessions::types::UnifiedSession;
use crate::sessions::{get_session_transcript, get_unified_sessions};

pub(super) fn search(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--query", "--harness", "--limit"],
        &["--content", "--json"],
    )?;

    let positional_q = args.first().filter(|a| !a.starts_with('-')).cloned();
    let query_str = options
        .one("--query")?
        .or(positional_q)
        .ok_or_else(|| usage("usage: orquestra sessions search <query> [--harness <id>] [--content] [--limit <n>] [--json]".to_string()))?;

    let harness = options.one("--harness")?;
    let search_content = options.has("--content");
    let limit = parse_u32(&options, "--limit")?.unwrap_or(15) as usize;
    let needle = query_str.to_lowercase();

    let mut sessions = get_unified_sessions().map_err(CliError::Operation)?;
    sessions.retain(|s| harness.as_deref().map_or(true, |h| s.harness == h));

    let mut matches: Vec<UnifiedSession> = Vec::new();

    for session in sessions {
        if matches.len() >= limit {
            break;
        }

        let meta_hit = session.title.to_lowercase().contains(&needle)
            || session
                .project_path
                .as_deref()
                .is_some_and(|p| p.to_lowercase().contains(&needle));

        if meta_hit {
            matches.push(session);
            continue;
        }

        if search_content {
            if let Ok(turns) = get_session_transcript(session.harness.clone(), session.id.clone()) {
                if turns
                    .iter()
                    .any(|t| t.content.to_lowercase().contains(&needle))
                {
                    matches.push(session);
                }
            }
        }
    }

    let human = if matches.is_empty() {
        format!("No sessions matching '{query_str}' found.")
    } else {
        let lines: Vec<String> = matches
            .iter()
            .map(|s| {
                let active = is_session_active(&s.harness, &s.id);
                format_session(s, active)
            })
            .collect();
        format!(
            "Search results for '{query_str}' ({} matches):\n{}",
            matches.len(),
            lines.join("\n")
        )
    };

    rendered(&matches, options.has("--json"), human)
}
