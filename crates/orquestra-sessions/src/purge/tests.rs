#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Destruction tests, split by what each case is protecting.
//!
//! Every fixture is built under `/var/tmp/testing` on the real store layout, so
//! a deletion is proven against the same paths, columns and lock files the
//! harnesses use. The two properties that matter most — a pinned session is
//! never deleted by accident, and nothing outside the harness store is touched
//! — are asserted on the filesystem, not on the report.
//!
//! This file holds the shared fixture, the module wiring and the purge entry
//! points every case module needs:
//!
//! * [`deletion`] — what a purge actually removes, and what it reports;
//! * [`per_harness`] — the removals each store's own layout obliges;
//! * [`safety`] — the guards that stop it removing the wrong thing;
//! * [`policy`] — when an automatic purge is due, and on whose terms.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::Store;
use crate::policy::toggle_pin_at;

/// Re-exported next to the fixture so a case module imports what it needs from
/// one place, `super`, instead of counting how many levels up `purge` sits.
pub(super) use super::{estimate, execute, execute_selection, select_by_ttl};

struct Fixture {
    home: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home = PathBuf::from("/var/tmp/testing")
            .join(format!("orquestra-purge-{}-{unique}", std::process::id()));
        let root = home.join(".orquestra");
        fs::create_dir_all(&root).expect("create the purge fixture under /var/tmp/testing");

        Self { home, root }
    }

    fn store(&self) -> Store {
        Store {
            home: self.home.clone(),
            root: self.root.clone(),
        }
    }

    fn write(&self, relative: &str, content: &str) -> u64 {
        let path = self.home.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create fixture parent");
        fs::write(&path, content).expect("write fixture file");
        content.len() as u64
    }

    fn database(&self, relative: &str, schema: &str) {
        let path = self.home.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create fixture parent");
        Connection::open(path)
            .expect("open fixture database")
            .execute_batch(schema)
            .expect("apply fixture schema");
    }

    /// Run SQL against a fixture database, for assertions after a purge.
    fn scalar(&self, relative: &str, sql: &str) -> i64 {
        Connection::open(self.home.join(relative))
            .expect("reopen fixture database")
            .query_row(sql, [], |row| row.get(0))
            .expect("query fixture database")
    }

    fn pin(&self, harness: &str, session_id: &str) {
        toggle_pin_at(
            &self.root,
            harness,
            session_id,
            Some("keep me".to_string()),
            "2026-10-18T06:00:00Z".to_string(),
        )
        .expect("pin the fixture session");
    }

    fn exists(&self, relative: &str) -> bool {
        self.home.join(relative).exists()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

/// An OpenCode store with one session per id and a sparse artifact each.
fn opencode_fixture(ids: &[&str]) -> Fixture {
    let fixture = Fixture::new();
    let rows = ids
        .iter()
        .map(|id| {
            format!(
                "INSERT INTO session VALUES ('{id}', 'Session {id}', '/work', 1789403075184, \
                 1789400000000);"
            )
        })
        .collect::<String>();
    fixture.database(
        ".local/share/opencode/opencode.db",
        &format!(
            "CREATE TABLE session (id TEXT PRIMARY KEY, title TEXT, directory TEXT,
                                   time_updated INTEGER, time_created INTEGER);
             {rows}"
        ),
    );

    for id in ids {
        fixture.write(
            &format!(".local/share/opencode/storage/session_diff/{id}/part.json"),
            "0123456789",
        );
    }

    fixture
}

fn adapter<'a>(
    adapters: &'a std::collections::BTreeMap<
        &'static str,
        Box<dyn crate::adapters::SessionAdapter>,
    >,
    harness: &str,
) -> &'a dyn crate::adapters::SessionAdapter {
    adapters
        .get(harness)
        .unwrap_or_else(|| panic!("no adapter for {harness}"))
        .as_ref()
}

mod deletion;
mod per_harness;
mod policy;
mod safety;
