#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Adapter discovery tests: the fixture harness and nothing else.
//!
//! Every fixture lives under `/var/tmp/testing` and mimics the real store
//! layout — including the columns the adapters actually query, so a schema
//! drift in these files means the query drifted too. The cases themselves
//! live in the sibling modules:
//!
//! * [`catalog_cases`] — listing, titles, turn counts and byte totals.
//! * [`transcript_cases`] — the transcript readers.
//! * [`store_guards`] — symlinks that must never be followed.
//! * [`stores`] and [`symlink_cases`] — per-harness store layouts and links.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{registry, SessionAdapter};

mod brain_links;
mod catalog_cases;
mod desktop_links;
mod grok_links;
mod id_escape;
mod store_guards;
mod stores;
mod symlink_cases;
mod transcript_cases;

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-sessions-adapters-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create the adapter test home under /var/tmp/testing");
        Self(path)
    }

    fn write(&self, relative: &str, content: &str) -> u64 {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create the fixture parent");
        fs::write(&path, content).expect("write the fixture file");
        content.len() as u64
    }

    fn database(&self, relative: &str, schema: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create the fixture parent");
        Connection::open(path)
            .expect("open the fixture database")
            .execute_batch(schema)
            .expect("apply the fixture schema");
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn adapter(home: &Path, harness: &str) -> Box<dyn SessionAdapter> {
    registry(home)
        .into_iter()
        .find(|adapter| adapter.harness_id() == harness)
        .unwrap_or_else(|| panic!("no adapter registered for {harness}"))
}
