//! Handoff brief generation subcommand.

use super::super::{parse_options, rendered, CliError};
use crate::sessions::types::UnifiedSession;
use crate::sessions::{get_session_transcript, get_unified_sessions};
use std::collections::BTreeSet;

pub(super) fn handoff(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--source-harness", "--harness", "--target-harness"],
        &["--json"],
    )?;

    let positional_id = args.first().filter(|a| !a.starts_with('-')).cloned();
    let source_harness = options
        .one("--source-harness")?
        .or(options.one("--harness")?);
    let target_harness = options
        .one("--target-harness")?
        .unwrap_or_else(|| "general".to_string());

    let sessions = get_unified_sessions().map_err(CliError::Operation)?;

    let session: UnifiedSession = match positional_id {
        Some(id) if id != "latest" => sessions
            .into_iter()
            .find(|s| s.id == id && source_harness.as_deref().map_or(true, |h| s.harness == h))
            .ok_or_else(|| CliError::Operation(format!("session '{id}' not found")))?,
        _ => sessions
            .into_iter()
            .find(|s| source_harness.as_deref().map_or(true, |h| s.harness == h))
            .ok_or_else(|| {
                CliError::Operation("no session found to generate handoff".to_string())
            })?,
    };

    let turns = get_session_transcript(session.harness.clone(), session.id.clone())
        .map_err(CliError::Operation)?;

    let mut files_touched = BTreeSet::new();
    let mut last_snippet = String::new();

    for turn in &turns {
        for word in turn.content.split_whitespace() {
            let clean =
                word.trim_matches(|c| c == '\'' || c == '"' || c == '`' || c == '(' || c == ')');
            if (clean.contains('/') || clean.contains('.'))
                && clean.len() > 3
                && !clean.starts_with("http")
                && (clean.ends_with(".rs")
                    || clean.ends_with(".ts")
                    || clean.ends_with(".tsx")
                    || clean.ends_with(".json")
                    || clean.ends_with(".md"))
            {
                files_touched.insert(clean.to_string());
            }
        }
        if !turn.content.trim().is_empty() {
            last_snippet = turn
                .content
                .trim()
                .lines()
                .take(5)
                .collect::<Vec<_>>()
                .join("\n");
        }
    }

    let files_list: Vec<String> = files_touched.into_iter().collect();
    let goal = session.title.clone();
    let blockers = "None identified".to_string();
    let next_steps =
        format!("Resume task execution in {target_harness} targeting recent workspace changes.");

    let report = serde_json::json!({
        "sessionId": session.id,
        "sourceHarness": session.harness,
        "targetHarness": target_harness,
        "goal": goal,
        "filesTouched": files_list,
        "blockers": blockers,
        "nextSteps": next_steps,
        "transcriptSnippet": last_snippet,
    });

    let files_rendered = if files_list.is_empty() {
        "  (None detected)".to_string()
    } else {
        files_list
            .iter()
            .map(|f| format!("  • {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let human = format!(
        "=== Handoff Brief ===\n\
         From:        {} (session {})\n\
         To:          {}\n\
         Goal:        {}\n\
         Blockers:    {}\n\
         Next Steps:  {}\n\
         Files Touched:\n{}\n\
         Latest Context:\n  {}",
        session.harness,
        session.id,
        target_harness,
        goal,
        blockers,
        next_steps,
        files_rendered,
        last_snippet.replace('\n', "\n  ")
    );

    rendered(&report, options.has("--json"), human)
}
