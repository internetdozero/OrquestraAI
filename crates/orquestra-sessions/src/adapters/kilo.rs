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

pub struct Kilo {
    root: PathBuf,
}

impl Kilo {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    fn store(&self) -> PathBuf {
        self.root.join(STORE_REL)
    }
}

impl SessionAdapter for Kilo {
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
        sqlite_store::read_transcript(&self.root, &format!("{STORE_REL}/{DATABASE}"), session_id)
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
