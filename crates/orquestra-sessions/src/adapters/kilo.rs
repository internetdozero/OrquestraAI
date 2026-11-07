//! Kilo Code: same storage lineage as OpenCode, one directory over.
//!
//! The schema is identical (`session`, `message`, `part` and the artifact
//! folders), so the shared reader does the work and this file only names the
//! store.

use std::path::{Path, PathBuf};

use crate::types::{TranscriptTurn, UnifiedSession};

use super::{sqlite_store, SessionAdapter};

const HARNESS: &str = "kilo";
const STORE_REL: &str = ".local/share/kilo";
const DATABASE: &str = "kilo.db";
/// The harness's own override: an exact database file, not a directory.
const DATABASE_OVERRIDE: &str = "KILO_DB";

pub struct Kilo {
    root: PathBuf,
}

impl Kilo {
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

impl SessionAdapter for Kilo {
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
