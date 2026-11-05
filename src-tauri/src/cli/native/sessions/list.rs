//! Sessions list subcommand.

use super::super::{parse_i64, parse_options, rendered, CliError};
use super::is_session_active;
use crate::sessions::get_unified_sessions;
use crate::sessions::types::UnifiedSession;

pub(super) fn list(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--harness", "--search", "--limit", "--offset"],
        &["--pinned-only", "--active-only", "--json"],
    )?;

    let harness = options.one("--harness")?;
    let search = options.one("--search")?.map(|s| s.to_lowercase());
    let limit = parse_i64(&options, "--limit")?.map(|l| l.max(0) as usize);
    let offset = parse_i64(&options, "--offset")?
        .map(|o| o.max(0) as usize)
        .unwrap_or(0);

    let mut sessions = get_unified_sessions().map_err(CliError::Operation)?;

    sessions.retain(|s| {
        let active = is_session_active(&s.harness, &s.id);
        harness.as_deref().map_or(true, |h| s.harness == h)
            && (!options.has("--pinned-only") || s.is_pinned)
            && (!options.has("--active-only") || active)
            && search.as_deref().map_or(true, |term| {
                s.title.to_lowercase().contains(term)
                    || s.project_path
                        .as_deref()
                        .is_some_and(|p| p.to_lowercase().contains(term))
            })
    });

    if offset > 0 {
        if offset >= sessions.len() {
            sessions.clear();
        } else {
            sessions.drain(..offset);
        }
    }

    if let Some(l) = limit {
        sessions.truncate(l);
    }

    let human = if sessions.is_empty() {
        "No matching sessions found.".to_string()
    } else {
        sessions
            .iter()
            .map(|s| {
                let active = is_session_active(&s.harness, &s.id);
                format_session(s, active)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    rendered(&sessions, options.has("--json"), human)
}

pub(super) fn format_session(session: &UnifiedSession, active: bool) -> String {
    let mut flags = Vec::new();
    if session.is_pinned {
        flags.push("pinned");
    }
    if active {
        flags.push("active");
    }
    let flag_str = if flags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", flags.join(", "))
    };

    format!(
        "[{}] {} — {} ({}){}",
        session.harness, session.id, session.title, session.updated_at, flag_str
    )
}
