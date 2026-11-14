#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Safety rails: the guards that stop a purge from removing something a
//! running agent still holds, or from reaching outside a harness store.

use std::fs;

use super::{adapter, estimate, execute, opencode_fixture, Fixture};
use crate::adapters::{adapters_by_id, fsops};
use crate::types::PurgeFilter;

// ---------------------------------------------------------------------------
// Lock and active-session guards
// ---------------------------------------------------------------------------

#[test]
fn copilot_refuses_to_touch_a_locked_session() {
    let fixture = Fixture::new();
    fixture.database(
        ".copilot/session-store.db",
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, cwd TEXT, repository TEXT, summary TEXT,
                                created_at TEXT, updated_at TEXT);
         CREATE TABLE turns (id INTEGER PRIMARY KEY, session_id TEXT, user_message TEXT,
                             assistant_response TEXT);
         INSERT INTO sessions VALUES ('locked', '/work', NULL, 'Busy', NULL,
             '2026-09-18T07:06:11.731Z');
         INSERT INTO turns VALUES (1, 'locked', 'hi', 'there');",
    );
    fixture.write(".copilot/session-state/locked/state.json", "12345");
    fixture.write(
        ".copilot/session-state/.session-operation-locks/locked.lock",
        "",
    );

    let estimate = estimate(&fixture.store(), &PurgeFilter::default()).expect("estimate");
    assert_eq!(estimate.sessions_count, 0);
    assert_eq!(estimate.skipped_active_count, 1);

    execute(&fixture.store(), &PurgeFilter::default()).expect("purge");
    assert!(fixture.exists(".copilot/session-state/locked/state.json"));
    assert_eq!(
        fixture.scalar(".copilot/session-store.db", "SELECT COUNT(*) FROM sessions"),
        1
    );
}

#[test]
fn grok_treats_a_listed_active_session_as_off_limits() {
    let fixture = Fixture::new();
    fixture.database(
        ".grok/sessions/session_search.sqlite",
        "CREATE TABLE session_docs (session_id TEXT PRIMARY KEY, cwd TEXT, updated_at INTEGER,
                                    title TEXT);
         INSERT INTO session_docs VALUES ('live', '/work', 1791708904, 'Live');",
    );
    fixture.write(".grok/sessions/-work/live/state.json", "123456");
    fixture.write(".grok/active_sessions.json", "[\"live\"]");

    let dry_run = estimate(&fixture.store(), &PurgeFilter::default()).expect("estimate");
    assert_eq!(dry_run.sessions_count, 0);
    assert_eq!(dry_run.skipped_active_count, 1);

    // Removing it from the active list makes the same session purgeable.
    fixture.write(".grok/active_sessions.json", "[]");
    assert_eq!(
        estimate(&fixture.store(), &PurgeFilter::default())
            .expect("estimate")
            .sessions_count,
        1
    );
}

#[test]
fn codex_reports_a_maintenance_lock_instead_of_deleting() {
    let fixture = Fixture::new();
    fixture.database(
        ".codex/thread_history_1.sqlite",
        "CREATE TABLE thread_turns (thread_id TEXT, turn_id TEXT, started_at INTEGER,
                                    completed_at INTEGER);
         INSERT INTO thread_turns VALUES ('thread-1', 't1', 1786241643, 1786241646);",
    );
    fixture.write(
        ".codex/sessions/2026/08/09/rollout-2026-08-09-01-thread-1.jsonl",
        "{}",
    );
    fixture.write(
        ".codex/session_index.jsonl",
        "{\"id\":\"thread-1\",\"thread_name\":\"Old\"}\n",
    );
    fixture.write(".codex/.sqlite-maintenance.lock", "");

    let adapters = adapters_by_id(&fixture.home);
    let error = adapter(&adapters, "codex")
        .delete_session("thread-1")
        .expect_err("a locked store must refuse");

    assert!(error.contains("maintenance"), "unexpected error: {error}");
    assert!(fixture.exists(".codex/sessions/2026/08/09/rollout-2026-08-09-01-thread-1.jsonl"));
    assert_eq!(
        fixture.scalar(
            ".codex/thread_history_1.sqlite",
            "SELECT COUNT(*) FROM thread_turns"
        ),
        1
    );

    // The same purge is reported as a failure rather than a silent success.
    let report = execute(
        &fixture.store(),
        &PurgeFilter {
            session_ids: Some(vec!["thread-1".to_string()]),
            ..PurgeFilter::default()
        },
    )
    .expect("purge");
    assert_eq!(report.deleted_count, 0);
    assert_eq!(report.failed_count, 1);
    assert!(report.errors[0].contains("codex/thread-1"));
}

#[test]
fn purge_never_writes_outside_the_harness_store() {
    let fixture = Fixture::new();
    let sentinel = fixture.write("outside/keep.txt", "keep me");

    // A traversal-shaped id must be refused, not resolved.
    let report = execute(
        &fixture.store(),
        &PurgeFilter {
            session_ids: Some(vec![
                "../../outside/keep.txt".to_string(),
                "/absolute/elsewhere".to_string(),
            ]),
            ..PurgeFilter::default()
        },
    )
    .expect("purge");

    assert_eq!(report.deleted_count, 0, "nothing matched those ids");
    assert!(fixture.exists("outside/keep.txt"));
    assert_eq!(
        fs::metadata(fixture.home.join("outside/keep.txt"))
            .unwrap()
            .len(),
        sentinel
    );

    // The path guard itself, called with a path that merely *starts with* the
    // store but climbs back out of it.
    let store = fixture.home.join("outside");
    let escape = store.join("../outside/keep.txt");
    assert!(
        escape.starts_with(&store),
        "this path is the case a prefix-only check would wave through"
    );
    let error = fsops::remove_path(&store, &escape).expect_err("traversal must be refused");
    assert!(error.contains("refusing to delete"), "unexpected: {error}");
    assert!(fixture.exists("outside/keep.txt"));
}

#[test]
fn a_purge_is_idempotent() {
    let fixture = opencode_fixture(&["ses_1"]);
    let filter = PurgeFilter::default();

    let first = execute(&fixture.store(), &filter).expect("first purge");
    let second = execute(&fixture.store(), &filter).expect("second purge");

    assert_eq!(first.deleted_count, 1);
    assert_eq!(second.deleted_count, 0);
    assert_eq!(
        second.failed_count, 0,
        "deleting what is gone is not a failure"
    );
    assert!(!fixture.exists(".local/share/opencode/storage/session_diff/ses_1"));
}

// ---------------------------------------------------------------------------
// Unreadable age
// ---------------------------------------------------------------------------

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
