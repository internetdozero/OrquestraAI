#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Deletion: what a purge removes, what it leaves, and what it reports.
//!
//! The per-harness removals — the ones that depend on each store's own layout —
//! live in [`per_harness`]; this file holds the generic cases that any store
//! has to satisfy, plus the dry run that must never delete anything.

use std::fs;

use super::super::{estimate, execute};
use super::{opencode_fixture, Fixture};
use crate::policy;
use crate::types::PurgeFilter;

mod per_harness;

// ---------------------------------------------------------------------------
// SQLite cascade and VACUUM
// ---------------------------------------------------------------------------

#[test]
fn deleting_a_session_cascades_to_children_and_vacuum_reclaims_the_file() {
    let fixture = Fixture::new();
    let blob = "x".repeat(400_000);
    fixture.database(
        ".local/share/opencode/opencode.db",
        &format!(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE session (id TEXT PRIMARY KEY, title TEXT, directory TEXT,
                                   time_updated INTEGER);
             CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL
                 REFERENCES session(id) ON DELETE CASCADE, body TEXT);
             CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL
                 REFERENCES message(id) ON DELETE CASCADE, text TEXT);
             INSERT INTO session VALUES ('ses_1', 'Big', '/work', 1789403075184);
             INSERT INTO message VALUES ('m1', 'ses_1', 'hello');
             INSERT INTO part VALUES ('p1', 'm1', '{blob}');
             INSERT INTO part VALUES ('p2', 'm1', '{blob}');"
        ),
    );

    let pages_before = fixture.scalar(".local/share/opencode/opencode.db", "PRAGMA page_count");

    let report = execute(
        &fixture.store(),
        &PurgeFilter {
            session_ids: Some(vec!["ses_1".to_string()]),
            ..PurgeFilter::default()
        },
    )
    .expect("purge");

    assert_eq!(report.deleted_count, 1);
    let database = ".local/share/opencode/opencode.db";
    assert_eq!(fixture.scalar(database, "SELECT COUNT(*) FROM session"), 0);
    assert_eq!(
        fixture.scalar(database, "SELECT COUNT(*) FROM message"),
        0,
        "foreign key cascade must remove child rows"
    );
    assert_eq!(fixture.scalar(database, "SELECT COUNT(*) FROM part"), 0);

    let pages_after = fixture.scalar(database, "PRAGMA page_count");
    assert!(
        pages_after < pages_before,
        "VACUUM should return pages to the filesystem ({pages_before} -> {pages_after})"
    );
}

#[test]
fn codex_removes_the_rollout_prunes_empty_days_and_rewrites_the_index() {
    let fixture = Fixture::new();
    fixture.database(
        ".codex/thread_history_1.sqlite",
        "CREATE TABLE thread_turns (thread_id TEXT, turn_id TEXT, started_at INTEGER,
                                    completed_at INTEGER);
         INSERT INTO thread_turns VALUES ('thread-1', 't1', 1786241643, 1786241646);
         INSERT INTO thread_turns VALUES ('thread-2', 't2', 1786241643, 1786241646);",
    );
    let kept = "{\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/work/two\"}}\n";
    fixture.write(
        ".codex/sessions/2026/08/09/rollout-2026-08-09-01-thread-1.jsonl",
        kept,
    );
    fixture.write(
        ".codex/sessions/2026/10/17/rollout-2026-10-17-02-thread-2.jsonl",
        kept,
    );
    fixture.write(
        ".codex/session_index.jsonl",
        "{\"id\":\"thread-1\",\"thread_name\":\"Old\"}\n{\"id\":\"thread-2\",\"thread_name\":\"Keep\"}\n",
    );

    let report = execute(
        &fixture.store(),
        &PurgeFilter {
            session_ids: Some(vec!["thread-1".to_string()]),
            ..PurgeFilter::default()
        },
    )
    .expect("purge");

    assert_eq!(report.deleted_count, 1);
    assert!(!fixture.exists(".codex/sessions/2026/08/09/rollout-2026-08-09-01-thread-1.jsonl"));
    assert!(
        !fixture.exists(".codex/sessions/2026/08"),
        "the emptied day and month folders should be pruned"
    );
    assert!(fixture.exists(".codex/sessions/2026/10/17/rollout-2026-10-17-02-thread-2.jsonl"));

    // The index must stay valid JSONL with no orphan entry.
    let index = fs::read_to_string(fixture.home.join(".codex/session_index.jsonl")).unwrap();
    let records: Vec<serde_json::Value> = index
        .lines()
        .map(|line| serde_json::from_str(line).expect("index lines stay parsable"))
        .collect();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["id"], "thread-2");

    assert_eq!(
        fixture.scalar(
            ".codex/thread_history_1.sqlite",
            "SELECT COUNT(*) FROM thread_turns"
        ),
        1
    );
}

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

// ---------------------------------------------------------------------------
// Surgical selection
// ---------------------------------------------------------------------------

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
// Empty run
// ---------------------------------------------------------------------------

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
