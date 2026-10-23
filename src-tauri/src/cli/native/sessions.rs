use super::{parse_options, parse_u32, rendered, usage, CliError};
use crate::sessions::types::{PurgeEstimate, PurgeFilter, PurgeReport, UnifiedSession};
use serde_json::json;

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("list") => list(&args[1..]),
        Some("purge") => purge(&args[1..]),
        _ => Err(usage(
            "usage: orquestra sessions <list|purge> [options]".to_string(),
        )),
    }
}

fn list(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--harness", "--search"],
        &["--json", "--pinned-only"],
    )?;
    let harness = options.one("--harness")?;
    let search = options.one("--search")?.map(|value| value.to_lowercase());
    let mut sessions = crate::sessions::get_unified_sessions().map_err(CliError::Operation)?;
    sessions.retain(|session| {
        harness
            .as_deref()
            .map_or(true, |value| session.harness == value)
            && (!options.has("--pinned-only") || session.is_pinned)
            && search.as_deref().map_or(true, |term| {
                session.title.to_lowercase().contains(term)
                    || session
                        .project_path
                        .as_deref()
                        .is_some_and(|path| path.to_lowercase().contains(term))
            })
    });
    let human = if sessions.is_empty() {
        "No sessions found.".to_string()
    } else {
        sessions
            .iter()
            .map(format_session)
            .collect::<Vec<_>>()
            .join("\n")
    };
    rendered(&sessions, options.has("--json"), human)
}

fn format_session(session: &UnifiedSession) -> String {
    let pinned = if session.is_pinned { " [pinned]" } else { "" };
    format!(
        "[{}] {} — {} ({}){}",
        session.harness, session.id, session.title, session.updated_at, pinned
    )
}

fn purge(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--harness", "--older-than-days", "--session-id"],
        &["--yes", "--json"],
    )?;
    let filter = PurgeFilter {
        harness: options.one("--harness")?,
        older_than_days: parse_u32(&options, "--older-than-days")?,
        session_ids: match options.repeated("--session-id") {
            ids if ids.is_empty() => None,
            ids => Some(ids),
        },
        ..PurgeFilter::default()
    };
    let execute = options.has("--yes");
    ensure_safe_purge(&filter, execute)?;

    if execute {
        let report = crate::sessions::purge_sessions(filter).map_err(CliError::Operation)?;
        rendered(
            &json!({ "dryRun": false, "report": report }),
            options.has("--json"),
            format_report(&report),
        )
    } else {
        let estimate = crate::sessions::estimate_purge(filter).map_err(CliError::Operation)?;
        rendered(
            &json!({ "dryRun": true, "estimate": estimate }),
            options.has("--json"),
            format_estimate(&estimate),
        )
    }
}

fn ensure_safe_purge(filter: &PurgeFilter, execute: bool) -> Result<(), CliError> {
    let has_selector = filter.harness.is_some()
        || filter.older_than_days.is_some()
        || filter
            .session_ids
            .as_ref()
            .is_some_and(|ids| !ids.is_empty());
    if execute && !has_selector {
        return Err(usage(
            "refusing unfiltered deletion; add --harness, --older-than-days, or --session-id"
                .to_string(),
        ));
    }
    Ok(())
}

fn format_estimate(estimate: &PurgeEstimate) -> String {
    format!(
        "Dry run: {} session(s), {} bytes; skipped {} pinned and {} active. Re-run with --yes and a filter to delete.",
        estimate.sessions_count,
        estimate.reclaimed_bytes,
        estimate.skipped_pinned_count,
        estimate.skipped_active_count
    )
}

fn format_report(report: &PurgeReport) -> String {
    let mut output = format!(
        "Deleted {} session(s), reclaimed {} bytes, {} failure(s).",
        report.deleted_count, report.reclaimed_bytes, report.failed_count
    );
    if !report.errors.is_empty() {
        output.push('\n');
        output.push_str(&report.errors.join("\n"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purge_requires_filter_for_execution_but_allows_dry_run() {
        let no_filter = PurgeFilter::default();
        assert!(ensure_safe_purge(&no_filter, false).is_ok());
        assert!(matches!(
            ensure_safe_purge(&no_filter, true),
            Err(CliError::Usage(_))
        ));
        let filtered = PurgeFilter {
            older_than_days: Some(30),
            ..PurgeFilter::default()
        };
        assert!(ensure_safe_purge(&filtered, true).is_ok());
    }
}
