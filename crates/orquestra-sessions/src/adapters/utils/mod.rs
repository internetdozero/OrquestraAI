//! Shared helpers for the harness adapters.
//!
//! Each concern has one module, all re-exported here so an adapter keeps
//! importing a single path:
//!
//! * [`sizing`] — recursive byte counts that never follow a symlink, because
//!   harness trees contain links to projects outside the home directory.
//!   Dependency, VCS and build-cache directories are pruned and descent is
//!   depth-capped, because worktrees contain trees no size column should pay
//!   to walk.
//! * [`text`] — title trimming and encoded-path recovery.
//! * [`json`] — JSON/JSONL readers that refuse symlinks and stream lines.
//! * [`sqlite`] — every open is read-only with `query_only` set, so listing
//!   sessions can never block an agent that is mid-conversation.
//! * [`super::time`] — harness clock dialects, re-exported below.

use std::path::PathBuf;

mod json;
mod sizing;
mod sqlite;
mod text;

pub use json::{
    first_json_line, for_each_jsonl, json_string, json_timestamp, message_text, read_json,
    read_jsonl, MAX_JSONL_BYTES,
};
pub(in crate::adapters) use sizing::regular_file_len;
pub use sizing::{dir_size, dir_size_limited, file_size, modified_secs};
pub use sqlite::{i64_cell, open_readonly, table_columns, table_exists, text_cell};
pub use text::{decode_path_hint, trim_title};

// Harness clock dialects live in `time`; re-exported so an adapter needs one
// import path for every helper it uses.
pub use super::time::{iso8601_from_secs, normalized_timestamp, now_secs, timestamp_secs};

/// The user's home directory. Adapters take it as a constructor argument so
/// tests can point them at a fixture tree instead.
pub fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set; cannot locate harness session stores".to_string())
}
