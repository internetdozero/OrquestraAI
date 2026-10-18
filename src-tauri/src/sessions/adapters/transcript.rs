//! Transcript reading: JSONL (and JSON-array) conversation logs normalized
//! to [`TranscriptTurn`].
//!
//! Split out of `utils.rs` so both files stay under the line cap. Parsing is
//! best-effort throughout: corrupt lines are skipped, system scaffolding,
//! tool payloads and thinking blocks are stripped, and oversized inputs are
//! refused whole, so a harness that is mid-write when the drawer opens can
//! never break the read.

use std::path::Path;

use serde_json::Value;

use crate::sessions::types::TranscriptTurn;

pub use super::transcript_clean::clean_turn_content;
use super::utils::{
    file_size, iso8601_from_secs, json_string, json_timestamp, normalized_timestamp,
};

/// Most turns a transcript read returns; a full replay is the drawer's job
/// to page, not the core's to dump in one payload. Shared with adapters
/// that page multi-file transcripts (Codex rollouts) themselves.
pub(crate) const MAX_TRANSCRIPT_TURNS: usize = 1000;
/// Longest single turn kept: long dumps are trimmed, never dropped whole.
const MAX_TURN_CHARS: usize = 12_000;
/// Refuse files above the shared JSONL guard: a runaway artifact must not
/// stall the UI thread.
const MAX_TRANSCRIPT_BYTES: u64 = super::utils::MAX_JSONL_BYTES;

/// Plain text of one transcript line's message, restricted to text blocks.
///
/// Accepts the `type` marker (Claude, Grok, Antigravity), the `kind` marker
/// (Freebuff Desktop parts) and Codex's `input_text` / `output_text` content
/// parts. Tool calls and tool results are excluded on purpose: they are
/// usually command output or JSON arguments, not conversation. A corrupt or
/// foreign shape yields `None`.
fn turn_text(message: &Value) -> Option<String> {
    match message {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Array(blocks) => {
            let mut joined = String::new();
            for block in blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                let kind = block
                    .get("type")
                    .or_else(|| block.get("kind"))
                    .and_then(Value::as_str)
                    .unwrap_or("text");
                if !matches!(kind, "text" | "input_text" | "output_text") {
                    continue;
                }
                let text = block
                    .get("text")
                    .or_else(|| block.get("content"))
                    .or_else(|| block.get("value"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if !text.is_empty() {
                    if !joined.is_empty() {
                        joined.push('\n');
                    }
                    joined.push_str(text);
                }
            }
            (!joined.is_empty()).then_some(joined)
        }
        Value::Object(map) => {
            // A nested envelope (`{"content": …}`) unwraps first; otherwise
            // the object itself may be one bare part
            // (`{"type": "text", "text": "…"}`, as OpenCode stores it).
            if let Some(nested) = map.get("content") {
                if let Some(text) = turn_text(nested) {
                    return Some(text);
                }
            }
            let kind = map
                .get("type")
                .or_else(|| map.get("kind"))
                .and_then(Value::as_str)
                .unwrap_or("text");
            if !matches!(kind, "text" | "input_text" | "output_text") {
                return None;
            }
            map.get("text")
                .or_else(|| map.get("value"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        }
        _ => None,
    }
}

/// Canonical role of one transcript line, or `None` to skip it.
///
/// Accepts the `role` field, the `source` marker Antigravity writes
/// (`USER_EXPLICIT`, `MODEL`) and the `type` marker (`USER_INPUT`,
/// `PLANNER_RESPONSE`, plus what Claude writes on every line). Anything else
/// — system prompts, developer instructions, summaries, mode latches,
/// permission records — is not conversation and is skipped. Grok marks its
/// synthetic scaffolding rows (`system_reminder`, `task_completed`) with
/// `synthetic_reason`, which is refused before the role is even read.
fn turn_role(entry: &Value) -> Option<&'static str> {
    if entry.get("synthetic_reason").is_some() {
        return None;
    }

    for key in ["role", "source", "type"] {
        let source = match key {
            "role" => entry
                .get("role")
                .or_else(|| entry.get("message")?.get("role")),
            "source" => entry.get("source"),
            _ => entry.get("type"),
        };
        if let Some(role) = source.and_then(Value::as_str) {
            return match role {
                "user" | "human" | "USER_EXPLICIT" | "USER_INPUT" => Some("user"),
                "assistant" | "model" | "ai" | "MODEL" | "PLANNER_RESPONSE" => {
                    Some("assistant")
                }
                // System prompts are harness scaffolding: they are not a
                // turn, and the drawer must never paint them as one.
                "system" | "developer" => return None,
                _ => continue,
            };
        }
    }
    None
}

/// Timestamp of one transcript line as ISO 8601 UTC, when recorded.
fn turn_timestamp(entry: &Value) -> Option<String> {
    if let Some(raw) = json_string(entry, &["timestamp", "published_at", "created_at"]) {
        return Some(normalized_timestamp(&raw));
    }
    json_timestamp(entry, &["timestamp", "published_at", "created_at"])
        .map(iso8601_from_secs)
}

/// Assemble one turn: system scaffolding refused, cleaned, empties dropped,
/// long dumps trimmed.
pub fn build_turn(
    role: &str,
    raw_content: &str,
    timestamp: Option<String>,
) -> Option<TranscriptTurn> {
    if role == "system" {
        return None;
    }
    let mut content = clean_turn_content(role, raw_content);
    if content.is_empty() {
        return None;
    }
    if content.chars().count() > MAX_TURN_CHARS {
        content = content.chars().take(MAX_TURN_CHARS).collect();
        content.push('…');
    }
    Some(TranscriptTurn {
        role: role.to_string(),
        content,
        timestamp,
    })
}

/// One parsed turn from a raw JSON value, or `None` when the line carries no
/// conversation text. Exposed for adapters (like Codex) whose lines wrap the
/// turn inside an envelope object rather than carrying it directly.
pub(crate) fn parse_value(entry: &Value) -> Option<TranscriptTurn> {
    let role = turn_role(entry)?;
    let message = entry.get("message").unwrap_or(entry);
    let raw = turn_text(message)
        .or_else(|| entry.get("content").and_then(turn_text))
        .or_else(|| json_string(entry, &["text", "input", "output"]))?;
    build_turn(role, &raw, turn_timestamp(entry))
}

/// Every conversation turn in a JSONL transcript file.
///
/// Corrupt lines are skipped (`filter_map`, never `unwrap`): a harness that
/// is mid-write when the drawer opens must not break the read. A file that
/// parses as a single JSON array instead (Manicode chat logs) is read
/// element-wise, so both layouts work without the caller caring.
pub fn read_transcript(path: &Path) -> Vec<TranscriptTurn> {
    if file_size(path) > MAX_TRANSCRIPT_BYTES {
        return Vec::new();
    }

    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut turns: Vec<TranscriptTurn> = raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|entry| parse_value(&entry))
        .take(MAX_TRANSCRIPT_TURNS)
        .collect();

    if turns.is_empty() {
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&raw) {
            turns = items
                .iter()
                .filter_map(parse_value)
                .take(MAX_TRANSCRIPT_TURNS)
                .collect();
        }
    }

    turns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_request_envelope_unwraps_to_the_prompt() {
        let raw = "<CONTEXT>x</CONTEXT>\n<USER_REQUEST>\n  fix the kernel\n</USER_REQUEST>\n<CONTEXT>y</CONTEXT>";
        assert_eq!(clean_turn_content("user", raw), "fix the kernel");
    }

    #[test]
    fn unclosed_user_request_keeps_the_remainder() {
        assert_eq!(
            clean_turn_content("user", "<USER_REQUEST>\n  hello"),
            "hello"
        );
    }

    #[test]
    fn plain_text_is_only_trimmed() {
        assert_eq!(clean_turn_content("assistant", "  on it\n"), "on it");
        assert_eq!(
            clean_turn_content("user", "no envelope here"),
            "no envelope here"
        );
    }

    #[test]
    fn thinking_blocks_are_stripped_for_every_role() {
        assert_eq!(
            clean_turn_content("assistant", "start <thinking>hmm</thinking> end"),
            "start  end"
        );
        assert_eq!(
            clean_turn_content("assistant", "<thought size=\"3\">x</thought>ok"),
            "ok"
        );
        assert_eq!(
            clean_turn_content("user", "keep <think>drop this"),
            "keep"
        );
    }

    #[test]
    fn tool_execution_records_never_become_speech() {
        let dump = "Created At: 2026-10-18T11:56:40Z\n\
                    Completed At: 2026-10-18T11:56:41Z\n\
                    Output:\nsrc-tauri/src/main.rs\n\
                    The command exited with code 0";
        assert!(build_turn("assistant", dump, None).is_none());
        assert!(build_turn("assistant", "The command exited with code 0", None).is_none());
        assert!(build_turn(
            "assistant",
            "Output:\nsrc-tauri/src/main.rs\nThe command exited with code 0",
            None
        )
        .is_none());

        let mixed = "I moved the cache.\n\nCreated At: 2026-10-18T11:56:40Z\n\
                     Output:\nls\nThe command exited with code 0";
        assert_eq!(
            build_turn("assistant", mixed, None)
                .expect("planner text survives the record")
                .content,
            "I moved the cache."
        );
    }

    #[test]
    fn prose_that_mentions_output_is_left_alone() {
        let prose = "Output: the command prints the resolved path";
        assert_eq!(clean_turn_content("assistant", prose), prose);
        let pasted = "Created At: when the record was written";
        assert_eq!(clean_turn_content("user", pasted), pasted);
    }

    #[test]
    fn grok_user_query_unwraps_and_drops_environment_rules() {
        assert_eq!(
            clean_turn_content(
                "user",
                "<user_info>cwd: /work</user_info>\n<user_query>\n  fix it\n</user_query>"
            ),
            "fix it"
        );
        assert!(clean_turn_content(
            "user",
            "<user_info>cwd: /work</user_info>\n<rules>be terse</rules>"
        )
        .is_empty());
        assert!(clean_turn_content("user", "<rules>be terse</rules>").is_empty());
    }

    #[test]
    fn synthetic_reason_rows_are_not_turns() {
        for (role, reason) in [
            ("user", "system_reminder"),
            ("assistant", "task_completed"),
        ] {
            let entry = serde_json::json!({
                "type": role,
                "content": "synthetic",
                "synthetic_reason": reason
            });
            assert!(parse_value(&entry).is_none(), "{reason} leaked");
        }
    }

    #[test]
    fn harness_tags_are_stripped() {
        let raw = "before <system-reminder>noise</system-reminder>\
                   <image_files>x.png</image_files>after <context>y</context>";
        assert_eq!(clean_turn_content("assistant", raw), "before after");
    }

    #[test]
    fn system_turns_are_never_conversation() {
        let entry = serde_json::json!({
            "role": "system",
            "content": "You are Grok 4.7 <dangerous_actions>…"
        });
        assert!(parse_value(&entry).is_none());
        assert!(build_turn("system", "You are Grok 4.7", None).is_none());
    }

    #[test]
    fn build_turn_drops_empties_and_trims_dumps() {
        assert!(build_turn("user", "   ", None).is_none());
        assert!(build_turn("user", "<thinking>only</thinking>", None).is_none());
        let long = "a".repeat(MAX_TURN_CHARS + 10);
        let turn = build_turn("assistant", &long, None).expect("trimmed, not dropped");
        assert_eq!(turn.content.chars().count(), MAX_TURN_CHARS + 1);
        assert!(turn.content.ends_with('…'));
    }
}
