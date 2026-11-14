//! The two Codex indexes: `thread_history_1.sqlite` for turn statistics and
//! `session_index.jsonl` for thread names. Both are optional — a thread can
//! exist only as a rollout file.

use std::collections::HashMap;
use std::io::BufRead;

use super::{Codex, DATABASE};
use crate::adapters::{time, utils};

/// `(created, updated, turns)` from the history database.
type TurnStats = (Option<i64>, Option<i64>, usize);

impl Codex {
    /// Thread names and index timestamps, keyed by thread id. Streamed into
    /// a small struct: the index never materializes as `Value` trees.
    pub(super) fn named_threads(&self) -> HashMap<String, (Option<String>, Option<i64>)> {
        #[derive(serde::Deserialize, Default)]
        struct IndexEntry {
            #[serde(default)]
            id: Option<String>,
            #[serde(default, rename = "thread_name")]
            thread_name: Option<String>,
            #[serde(default)]
            title: Option<String>,
            #[serde(default, rename = "updated_at")]
            updated_at: Option<IndexTs>,
        }
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum IndexTs {
            Num(f64),
            Str(String),
        }

        let mut named = HashMap::new();
        let path = self.store().join("session_index.jsonl");
        if utils::regular_file_len(&path).is_none() {
            return named;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            return named;
        };
        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(entry) = serde_json::from_str::<IndexEntry>(line) else {
                continue;
            };
            let Some(id) = entry.id.as_deref() else {
                continue;
            };
            let name = entry
                .thread_name
                .as_deref()
                .or(entry.title.as_deref())
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string);
            let updated = entry.updated_at.as_ref().and_then(|at| match at {
                IndexTs::Num(number) => Some(time::scale_epoch(*number)),
                IndexTs::Str(text) => time::timestamp_secs(text),
            });
            named.insert(id.to_string(), (name, updated));
        }
        named
    }

    /// Turn counts and activity window from the read-only history database.
    /// A missing database is normal; a corrupt one fails this harness only.
    pub(super) fn turn_stats(&self) -> Result<HashMap<String, TurnStats>, String> {
        let database = self.store().join(DATABASE);
        if !database.is_file() {
            return Ok(HashMap::new());
        }

        let connection = utils::open_readonly(&database)?;
        if !utils::table_exists(&connection, "thread_turns")? {
            return Ok(HashMap::new());
        }

        let mut statement = connection
            .prepare(
                "SELECT thread_id,
                        MIN(COALESCE(started_at, completed_at)),
                        MAX(COALESCE(completed_at, started_at)),
                        COUNT(*)
                 FROM thread_turns
                 GROUP BY thread_id",
            )
            .map_err(|error| format!("{}: {error}", database.display()))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    utils::text_cell(row, 0),
                    utils::i64_cell(row, 1),
                    utils::i64_cell(row, 2),
                    utils::i64_cell(row, 3),
                ))
            })
            .map_err(|error| format!("{}: {error}", database.display()))?;

        Ok(rows
            .flatten()
            .filter_map(|(id, created, updated, turns)| {
                Some((id?, (created, updated, turns.unwrap_or(0).max(0) as usize)))
            })
            .collect())
    }
}
