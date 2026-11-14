//! OpenCode: one SQLite database under `~/.local/share/opencode`.
//!
//! Titles, working directories and update times come from the `session` table;
//! per-session diffs and tool output live beside it in artifact folders.

use std::path::{Path, PathBuf};

use crate::types::{TranscriptTurn, UnifiedSession};

use super::{sqlite_store, SessionAdapter};

const HARNESS: &str = "opencode";
const STORE_REL: &str = ".local/share/opencode";
const DATABASE: &str = "opencode.db";
/// The harness's own override: an exact database file, not a directory.
const DATABASE_OVERRIDE: &str = "OPENCODE_DB";

pub struct OpenCode {
    root: PathBuf,
}

impl OpenCode {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    /// Store directory and database file, resolved for this platform.
    fn store_paths(&self) -> (PathBuf, PathBuf) {
        sqlite_store::store_paths(
            &self.root,
            STORE_REL,
            DATABASE,
            std::env::var_os(DATABASE_OVERRIDE),
        )
    }
}

impl SessionAdapter for OpenCode {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        self.store_paths().1.is_file()
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let (store, database) = self.store_paths();
        sqlite_store::list(&store, &database, HARNESS)
    }

    fn read_transcript(&self, session_id: &str) -> Vec<TranscriptTurn> {
        sqlite_store::read_transcript(&self.store_paths().1, session_id)
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        let (store, database) = self.store_paths();
        sqlite_store::delete(&store, &database, session_id)
    }

    fn optimize_storage(&self) -> Result<(), String> {
        sqlite_store::vacuum(&self.store_paths().1)
    }
}
