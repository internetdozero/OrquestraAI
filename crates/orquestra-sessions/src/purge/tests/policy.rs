#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Policy: when an automatic purge becomes due, how each harness's own TTL is
//! honoured, and why a scheduled run is still subject to the pin guard.

use super::{execute_selection, opencode_fixture, select_by_ttl, Fixture};
use crate::scheduler;
use crate::types::RetentionPolicy;

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
        last_run: Some(crate::adapters::time::iso8601_from_secs(1_000_000)),
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
    let report = execute_selection(&fixture.store(), selection).expect("purge");

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
