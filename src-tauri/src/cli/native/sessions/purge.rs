//! Session purge estimation and execution subcommand.

use super::super::{parse_options, parse_u32, rendered, usage, CliError};
use crate::sessions::types::{PurgeEstimate, PurgeFilter, PurgeReport};
use crate::sessions::{estimate_purge, purge_sessions};
use serde_json::json;

pub(super) fn purge(args: &[String]) -> Result<String, CliError> {
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
        let report = purge_sessions(filter).map_err(CliError::Operation)?;
        rendered(
            &json!({ "dryRun": false, "report": report }),
            options.has("--json"),
            format_report(&report),
        )
    } else {
        let estimate = estimate_purge(filter).map_err(CliError::Operation)?;
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
            "refusing unbounded purge without a selector: pass --older-than-days, --harness, or --session-id".to_string(),
        ));
    }
    Ok(())
}

fn format_estimate(estimate: &PurgeEstimate) -> String {
    format!(
        "Dry run: {} sessions would be deleted ({} bytes reclaimed across {:?}); \
         {} skipped because pinned, {} skipped because active.",
        estimate.sessions_count,
        estimate.reclaimed_bytes,
        estimate.affected_harnesses,
        estimate.skipped_pinned_count,
        estimate.skipped_active_count,
    )
}

fn format_report(report: &PurgeReport) -> String {
    let mut message = format!(
        "Deleted {} sessions, reclaimed {} bytes; {} failed.",
        report.deleted_count, report.reclaimed_bytes, report.failed_count,
    );
    for error in &report.errors {
        message.push_str(&format!("\n  error: {error}"));
    }
    message
}
