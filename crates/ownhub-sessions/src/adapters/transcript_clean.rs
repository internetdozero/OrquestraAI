//! Per-turn text cleaning shared by every transcript reader.
//!
//! Harnesses wrap operator prompts in envelopes (Antigravity's
//! `<USER_REQUEST>`, Grok's `<user_query>`) and prepend environment
//! scaffolding (`<user_info>`, `<rules>`). Cleaning unwraps the real prompt,
//! drops scaffolding whole, and strips harness-only tags. Split from
//! `transcript.rs` so both files stay under the cap.

use super::transcript_sanitize::strip_tool_run;

/// Drop one `<tag …>…</tag>` span family from already-extracted text.
/// Closed spans are removed; an unclosed opener drops everything after it,
/// because a dangling thinking block is truncated reasoning, never content.
fn strip_tag(text: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find(open.as_str()) {
        let Some(end_of_open) = rest[start..].find('>') else {
            break;
        };
        let after_open = start + end_of_open + 1;
        out.push_str(&rest[..start]);
        if let Some(end) = rest[after_open..].find(close.as_str()) {
            rest = &rest[after_open + end + close.len()..];
        } else {
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Inner text of the first `<open>…</open>` span, when the opener is present.
/// An unclosed opener keeps the remainder: a truncated envelope must not
/// swallow the prompt it carries.
fn unwrap_tag(text: &str, open: &str, close: &str) -> Option<String> {
    let start = text.find(open)?;
    let inner = &text[start + open.len()..];
    let end = inner
        .find(close)
        .map(|index| &inner[..index])
        .unwrap_or(inner);
    Some(end.trim().to_string())
}

/// Clean raw turn text for display: user prompts wrapped in Antigravity's
/// `<USER_REQUEST>` or Grok's `<user_query>` envelope are unwrapped to the
/// actual prompt, environment scaffolding is dropped whole, thinking blocks
/// and harness tags are stripped for every role, and assistant text drops any
/// tool-execution record a harness appended to it.
pub fn clean_turn_content(role: &str, raw: &str) -> String {
    let mut text = raw.trim().to_string();

    if role == "user" {
        if let Some(inner) = unwrap_tag(&text, "<USER_REQUEST>", "</USER_REQUEST>") {
            text = inner;
        }
        if let Some(inner) = unwrap_tag(&text, "<user_query>", "</user_query>") {
            text = inner;
        } else if text.contains("<user_info>") || text.contains("<rules>") {
            // Grok's environment preamble: instructions to the model, not a
            // message from the operator.
            return String::new();
        }
    }

    for tag in [
        "thought",
        "thinking",
        "think",
        "system-reminder",
        "image_files",
        "context",
    ] {
        text = strip_tag(&text, tag);
    }

    if role != "user" {
        text = strip_tool_run(&text);
    }

    text.trim().to_string()
}
