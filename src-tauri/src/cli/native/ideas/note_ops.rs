//! Operations on blackboard notes: note-post, note-list, note-get, note-read, note-clear.

use super::super::{parse_i64, parse_options, rendered, usage, CliError};
use super::render::{render_note_detail, render_notes_table};
use crate::ideas::types::PostNoteInput;
use crate::ideas::{
    clear_blackboard_notes, get_note, list_blackboard_notes, mark_note_read, post_blackboard_note,
};

pub(super) fn note_post(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--project", "--topic", "--note", "--sender", "--target"],
        &["--json"],
    )?;

    let project = options.required("--project")?;
    let topic = options.required("--topic")?;
    let note = options.required("--note")?;
    let sender = options
        .one("--sender")?
        .unwrap_or_else(|| "operator".to_string());
    let target = options.one("--target")?;

    let post = post_blackboard_note(PostNoteInput {
        project,
        topic,
        sender,
        target,
        note,
    })
    .map_err(CliError::Operation)?;

    let human = format!(
        "Posted Note #{} [{}] to project '{}'.",
        post.id, post.topic, post.project
    );
    rendered(&post, options.has("--json"), human)
}

pub(super) fn note_list(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--project", "--topic", "--limit", "--offset"],
        &["--unread-only", "--json"],
    )?;

    let project = options.one("--project")?;
    let unread_only = options.has("--unread-only");
    let limit = parse_i64(&options, "--limit")?;
    let offset = parse_i64(&options, "--offset")?;

    let mut notes =
        list_blackboard_notes(project, unread_only, limit, offset).map_err(CliError::Operation)?;

    if let Some(topic) = options.one("--topic")? {
        let needle = topic.to_lowercase();
        notes.retain(|n| n.topic.to_lowercase() == needle);
    }

    let human = render_notes_table(&notes);
    rendered(&notes, options.has("--json"), human)
}

pub(super) fn note_get(args: &[String]) -> Result<String, CliError> {
    let id_str = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: ownhub idea note-get <id> [--json]".to_string()))?;

    let id = id_str
        .parse::<i64>()
        .map_err(|_| usage(format!("invalid note ID '{id_str}', expected integer")))?;

    let options = parse_options(&args[1..], &[], &["--json"])?;
    let note = get_note(id).map_err(CliError::Operation)?;
    let human = render_note_detail(&note);
    rendered(&note, options.has("--json"), human)
}

pub(super) fn note_read(args: &[String]) -> Result<String, CliError> {
    let id_str = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: ownhub idea note-read <id> [--json]".to_string()))?;

    let id = id_str
        .parse::<i64>()
        .map_err(|_| usage(format!("invalid note ID '{id_str}', expected integer")))?;

    let options = parse_options(&args[1..], &[], &["--json"])?;
    mark_note_read(id).map_err(CliError::Operation)?;

    let report = serde_json::json!({ "id": id, "isRead": true });
    let human = format!("Marked note #{id} as read.");
    rendered(&report, options.has("--json"), human)
}

pub(super) fn note_clear(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--project", "--topic"],
        &["--include-unread", "--json"],
    )?;

    let project = options.required("--project")?;
    let topic = options.one("--topic")?;
    let include_unread = options.has("--include-unread");

    let count =
        clear_blackboard_notes(&project, topic, include_unread).map_err(CliError::Operation)?;

    let report = serde_json::json!({ "project": project, "cleared": count });
    let human = format!("Cleared {count} notes from blackboard for '{project}'.");
    rendered(&report, options.has("--json"), human)
}
