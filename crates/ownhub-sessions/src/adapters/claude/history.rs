//! `history.jsonl`: one line per submitted prompt, aggregated into the title,
//! the recorded project and the activity window the listing shows.

use std::collections::HashMap;
use std::io::BufRead;

use super::Claude;
use crate::adapters::{time, utils};

/// Prompts recorded for one session in `history.jsonl`.
#[derive(Default)]
pub(super) struct History {
    pub(super) title: Option<String>,
    pub(super) project: Option<String>,
    pub(super) created: Option<i64>,
    pub(super) updated: Option<i64>,
    pub(super) prompts: usize,
}

/// Timestamps arrive as epoch numbers or ISO strings depending on release.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum HistoryTs {
    Num(f64),
    Str(String),
}

impl HistoryTs {
    fn as_secs(&self) -> Option<i64> {
        match self {
            HistoryTs::Num(number) => Some(time::scale_epoch(*number)),
            HistoryTs::Str(text) => time::timestamp_secs(text),
        }
    }
}

/// One `history.jsonl` line: only the fields the listing reads. Unknown
/// fields are ignored, so one line parses to a few hundred bytes — never a
/// full `Value` tree with its embedded payloads.
#[derive(serde::Deserialize, Default)]
struct HistoryEntry {
    #[serde(rename = "sessionId", default)]
    session_id: Option<String>,
    #[serde(default)]
    display: Option<String>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    timestamp: Option<HistoryTs>,
    #[serde(default, rename = "created_at")]
    created_at: Option<HistoryTs>,
}

impl Claude {
    /// Streamed line-by-line into [`HistoryEntry`]: the index stays a few
    /// kilobytes of map no matter how long the session history grows.
    pub(super) fn history(&self) -> HashMap<String, History> {
        let mut history: HashMap<String, History> = HashMap::new();
        let path = self.store().join("history.jsonl");
        if utils::regular_file_len(&path).is_none() {
            return history;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            return history;
        };

        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) else {
                continue;
            };
            let Some(session) = entry.session_id.as_deref() else {
                continue;
            };

            let record = history.entry(session.to_string()).or_default();
            record.prompts += 1;

            let useful = entry
                .display
                .as_deref()
                .filter(|text| !text.trim().is_empty() && !text.starts_with('/'));
            if record.title.is_none() {
                record.title = useful.map(utils::trim_title);
            }
            if record.project.is_none() {
                record.project = entry.project.as_deref().and_then(|raw| {
                    let text = raw.trim();
                    (!text.is_empty()).then(|| text.to_string())
                });
            }
            let at = entry
                .timestamp
                .as_ref()
                .and_then(HistoryTs::as_secs)
                .or_else(|| entry.created_at.as_ref().and_then(HistoryTs::as_secs));
            if let Some(at) = at {
                record.created = Some(record.created.map_or(at, |known: i64| known.min(at)));
                record.updated = Some(record.updated.map_or(at, |known: i64| known.max(at)));
            }
        }

        history
    }
}
