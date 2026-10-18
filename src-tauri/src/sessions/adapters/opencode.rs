//! OpenCode: one SQLite database under `~/.local/share/opencode`.
//!
//! Titles, working directories and update times come from the `session` table;
//! per-session diffs and tool output live beside it in artifact folders.

use std::path::{Path, PathBuf};

use crate::sessions::types::{TranscriptTurn, UnifiedSession};

use super::{sqlite_store, SessionAdapter};

const HARNESS: &str = "opencode";
const STORE_REL: &str = ".local/share/opencode";
const DATABASE: &str = "opencode.db";

pub struct OpenCode {
    root: PathBuf,
}

impl OpenCode {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    fn store(&self) -> PathBuf {
        self.root.join(STORE_REL)
    }
}

impl SessionAdapter for OpenCode {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        self.store().is_dir()
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        sqlite_store::list(
            &self.root,
            HARNESS,
            &format!("{STORE_REL}/{DATABASE}"),
            STORE_REL,
        )
    }

    fn read_transcript(&self, session_id: &str) -> Vec<TranscriptTurn> {
        sqlite_store::read_transcript(
            &self.root,
            &format!("{STORE_REL}/{DATABASE}"),
            session_id,
        )
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        sqlite_store::delete(
            &self.root,
            &format!("{STORE_REL}/{DATABASE}"),
            STORE_REL,
            session_id,
        )
    }

    fn optimize_storage(&self) -> Result<(), String> {
        sqlite_store::vacuum(&self.root, &format!("{STORE_REL}/{DATABASE}"))
    }
}
