//! JSON access for harness stores: whole documents, single headers, and
//! streamed JSONL transcripts. Every reader refuses a symlink, so a session
//! file cannot be measured or loaded from outside the store.

use std::fs;
use std::path::Path;

use serde_json::Value;

use super::sizing::regular_file_len;
use crate::adapters::time::{scale_epoch, timestamp_secs};

/// Refuse to load a JSONL index bigger than this; such a file is a runaway
/// artifact, and parsing it would cost more than the listing is worth.
pub const MAX_JSONL_BYTES: u64 = 32 * 1024 * 1024;

/// Read a JSON document, treating a missing or corrupt file as "nothing there".
pub fn read_json(path: &Path) -> Option<Value> {
    regular_file_len(path)?;
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Parse the first JSONL line without loading the whole transcript: rollout
/// files run to hundreds of megabytes and only the header matters.
pub fn first_json_line(path: &Path) -> Option<Value> {
    use std::io::BufRead;

    regular_file_len(path)?;
    let file = fs::File::open(path).ok()?;
    let mut line = String::new();
    std::io::BufReader::new(file).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

/// Every parsable line of a JSONL index, streamed one line at a time.
///
/// The file is never held whole in memory: each line is parsed and moved
/// into the output before the next is read, so the peak is one line, not
/// the file. Corrupt lines are skipped; symlinks are refused by
/// [`regular_file_len`]. There is no size gate — callers that only need the
/// first entries should use [`for_each_jsonl`] to stop early instead.
pub fn read_jsonl(path: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    for_each_jsonl(path, &mut |entry| {
        out.push(entry);
        true
    });
    out
}

/// Stream parsed JSONL lines, stopping early when `visit` returns `false`.
///
/// Rollout and transcript files run to tens of megabytes; collecting them
/// whole just to keep the first thousand turns peaked at 150–300MB of RAM
/// (raw string plus a `Value` per line). This keeps one line alive at a
/// time, so a 50MB file parses under a few megabytes of working set plus
/// whatever the visitor retains.
pub fn for_each_jsonl(path: &Path, visit: &mut dyn FnMut(Value) -> bool) {
    use std::io::BufRead;

    if regular_file_len(path).is_none() {
        return;
    }
    let Ok(file) = fs::File::open(path) else {
        return;
    };
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<Value>(line) {
            if !visit(entry) {
                break;
            }
        }
    }
}

/// First non-empty string among `keys`, so one helper covers `title`,
/// `thread_name`, `summary` and friends.
pub fn json_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(text) = value.get(*key).and_then(Value::as_str) else {
            continue;
        };
        let text = text.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    None
}

/// First parsable timestamp among `keys`, accepting numbers and strings.
pub fn json_timestamp(value: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        let Some(field) = value.get(*key) else {
            continue;
        };

        match field {
            Value::Number(number) => {
                if let Some(number) = number.as_f64() {
                    return Some(scale_epoch(number));
                }
            }
            Value::String(text) => {
                if let Some(secs) = timestamp_secs(text) {
                    return Some(secs);
                }
            }
            _ => {}
        }
    }

    None
}

/// Human text of a message body: a plain string, a list of `{"text": ...}`
/// blocks, or an object with a `content` field holding either.
pub fn message_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Array(blocks) => {
            blocks
                .iter()
                .filter_map(message_text)
                .reduce(|mut joined, text| {
                    joined.push(' ');
                    joined.push_str(&text);
                    joined
                })
        }
        Value::Object(map) => map.get("content").and_then(message_text),
        _ => None,
    }
}
