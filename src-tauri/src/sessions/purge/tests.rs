#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Destruction tests.
//!
//! Every fixture is built under `/var/tmp/testing` on the real store layout, so
//! a deletion is proven against the same paths, columns and lock files the
//! harnesses use. The two properties that matter most — a pinned session is
//! never deleted by accident, and nothing outside the harness store is touched
//! — are asserted on the filesystem, not on the report.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{estimate, execute, select_by_ttl, Store};

use crate::sessions::policy;
use crate::sessions::scheduler;
use crate::sessions::types::{PurgeFilter, RetentionPolicy};

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
        policy::toggle_pin_at(
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
        Box<dyn crate::sessions::adapters::SessionAdapter>,
    >,
    harness: &str,
) -> &'a dyn crate::sessions::adapters::SessionAdapter {
    adapters
        .get(harness)
        .unwrap_or_else(|| panic!("no adapter for {harness}"))
        .as_ref()
}

// ---------------------------------------------------------------------------
// Dry run
// ---------------------------------------------------------------------------

mod deletion;
mod safety;

// ---------------------------------------------------------------------------
// Dry run
// ---------------------------------------------------------------------------

#[test]
fn estimate_reports_bytes_and_deletes_nothing() {
    let fixture = opencode_fixture(&["ses_1", "ses_2"]);
    let filter = PurgeFilter {
        older_than_days: Some(7),
        ..PurgeFilter::default()
    };

    let estimate = estimate(&fixture.store(), &filter).expect("estimate");

    assert_eq!(estimate.sessions_count, 2);
    assert_eq!(estimate.affected_harnesses, vec!["opencode".to_string()]);
    assert!(
        estimate.reclaimed_bytes >= 20,
        "both artifacts should be measured"
    );
    assert_eq!(estimate.skipped_pinned_count, 0);

    // The whole point of a dry run.
    assert!(fixture.exists(".local/share/opencode/storage/session_diff/ses_1/part.json"));
    assert_eq!(
        fixture.scalar(
            ".local/share/opencode/opencode.db",
            "SELECT COUNT(*) FROM session"
        ),
        2
    );
}

#[test]
fn a_session_with_no_readable_age_is_never_age_purged() {
    let fixture = Fixture::new();
    fixture.database(
        ".local/share/opencode/opencode.db",
        "CREATE TABLE session (id TEXT, title TEXT, directory TEXT, time_updated INTEGER);
         INSERT INTO session VALUES ('fresh', 'Fresh', '/work', NULL);",
    );

    let aged = estimate(
        &fixture.store(),
        &PurgeFilter {
            older_than_days: Some(1),
            ..PurgeFilter::default()
        },
    )
    .expect("estimate");
    assert_eq!(aged.sessions_count, 0, "no timestamp means no known age");

    // Without an age rule the same session is fair game.
    let all = estimate(&fixture.store(), &PurgeFilter::default()).expect("estimate");
    assert_eq!(all.sessions_count, 1);
}

// ---------------------------------------------------------------------------
// Pin protection
// ---------------------------------------------------------------------------

#[test]
fn a_pinned_session_survives_a_purge_that_deletes_its_neighbours() {
    let fixture = opencode_fixture(&["ses_1", "ses_2"]);
    fixture.pin("opencode", "ses_1");

    let filter = PurgeFilter {
        older_than_days: Some(7),
        ..PurgeFilter::default()
    };
    let report = execute(&fixture.store(), &filter).expect("purge");

    assert_eq!(report.deleted_count, 1);
    assert_eq!(report.failed_count, 0);

    assert!(fixture.exists(".local/share/opencode/storage/session_diff/ses_1/part.json"));
    assert!(!fixture.exists(".local/share/opencode/storage/session_diff/ses_2/part.json"));
    assert_eq!(
        fixture.scalar(
            ".local/share/opencode/opencode.db",
            "SELECT COUNT(*) FROM session WHERE id = 'ses_1'"
        ),
        1
    );
    assert_eq!(
        fixture.scalar(
            ".local/share/opencode/opencode.db",
            "SELECT COUNT(*) FROM session"
        ),
        1
    );

    // The operator is told what was protected, not left guessing.
    let dry = estimate(&fixture.store(), &filter).expect("estimate");
    assert_eq!(dry.skipped_pinned_count, 1);
}

#[test]
fn an_explicit_request_can_delete_a_pinned_session() {
    let fixture = opencode_fixture(&["ses_1"]);
    fixture.pin("opencode", "ses_1");

    let report = execute(
        &fixture.store(),
        &PurgeFilter {
            ignore_pinned: false,
            ..PurgeFilter::default()
        },
    )
    .expect("purge");

    assert_eq!(report.deleted_count, 1);
    assert_eq!(
        fixture.scalar(
            ".local/share/opencode/opencode.db",
            "SELECT COUNT(*) FROM session"
        ),
        0
    );
}

#[test]
fn a_surgical_id_list_deletes_only_that_session() {
    let fixture = opencode_fixture(&["ses_1", "ses_2", "ses_3"]);

    let report = execute(
        &fixture.store(),
        &PurgeFilter {
            session_ids: Some(vec!["ses_2".to_string()]),
            ..PurgeFilter::default()
        },
    )
    .expect("purge");

    assert_eq!(report.deleted_count, 1);
    assert!(fixture.exists(".local/share/opencode/storage/session_diff/ses_1/part.json"));
    assert!(!fixture.exists(".local/share/opencode/storage/session_diff/ses_2/part.json"));
    assert!(fixture.exists(".local/share/opencode/storage/session_diff/ses_3/part.json"));
}

// ---------------------------------------------------------------------------
// Scheduler decisions
// ---------------------------------------------------------------------------

#[test]
fn auto_purge_is_due_only_when_enabled_and_the_interval_has_passed() {
    let day = 86_400;

    let disabled = RetentionPolicy {
        auto_purge_enabled: false,
        ..RetentionPolicy::default()
    };
    assert!(!scheduler::due(&disabled, 0));

    let never_run = RetentionPolicy {
        auto_purge_enabled: true,
        ..RetentionPolicy::default()
    };
    assert!(
        scheduler::due(&never_run, 1_000_000),
        "a fresh policy is due"
    );

    let recent = RetentionPolicy {
        auto_purge_enabled: true,
        last_run: Some(crate::sessions::adapters::time::iso8601_from_secs(
            1_000_000,
        )),
        ..RetentionPolicy::default()
    };
    assert!(!scheduler::due(&recent, 1_000_000 + 3600));

    let elapsed = RetentionPolicy {
        auto_purge_interval_hours: 24,
        ..recent.clone()
    };
    assert!(scheduler::due(&elapsed, 1_000_000 + day));
}

#[test]
fn the_scheduler_uses_each_harness_own_ttl() {
    let fixture = Fixture::new();
    // Same age, two harnesses.
    let sql = "CREATE TABLE session (id TEXT, title TEXT, directory TEXT, time_updated INTEGER);";
    fixture.database(
        ".local/share/opencode/opencode.db",
        &format!("{sql} INSERT INTO session VALUES ('old', 'Old', '/work', 1789403075184);"),
    );
    fixture.database(
        ".local/share/kilo/kilo.db",
        &format!("{sql} INSERT INTO session VALUES ('old', 'Old', '/work', 1789403075184);"),
    );

    // Kilo keeps 30 days, OpenCode only 7: the session is old enough for
    // OpenCode and far too recent for Kilo.
    let selection = select_by_ttl(&fixture.store(), &|harness| {
        if harness == "kilo" {
            60
        } else {
            7
        }
    })
    .expect("select");

    assert_eq!(selection.candidates.len(), 1);
    assert_eq!(selection.candidates[0].harness, "opencode");
}

#[test]
fn a_scheduled_purge_still_honours_pins() {
    let fixture = opencode_fixture(&["ses_1", "ses_2"]);
    fixture.pin("opencode", "ses_1");

    let selection = select_by_ttl(&fixture.store(), &|_| 7).expect("select");
    let report = super::execute_selection(&fixture.store(), selection).expect("purge");

    assert_eq!(report.deleted_count, 1);
    assert!(fixture.exists(".local/share/opencode/storage/session_diff/ses_1/part.json"));
    assert_eq!(
        fixture.scalar(
            ".local/share/opencode/opencode.db",
            "SELECT COUNT(*) FROM session WHERE id = 'ses_1'"
        ),
        1
    );
}

#[test]
fn an_empty_purge_still_records_the_run() {
    let fixture = opencode_fixture(&[]);
    let report = execute(&fixture.store(), &PurgeFilter::default()).expect("purge");

    assert_eq!(report.deleted_count, 0);
    let policy = policy::load_from(&fixture.root).expect("policy sidecar");
    assert!(
        policy.retention.last_run.is_some(),
        "the scheduler needs the run recorded to advance its interval"
    );
}
