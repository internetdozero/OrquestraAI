//! Tests for the pure layers of the session catalog: timestamp and title
//! handling, the atomic policy sidecar, and the frozen payload shapes.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use super::adapters::utils;
use super::policy::{self, PolicyFile};
use super::types::{PinnedSession, PurgeReport, RetentionPolicy, SessionFilter, UnifiedSession};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-sessions-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .expect("create the sessions test workspace under /var/tmp/testing");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn keys_of<T: Serialize>(value: &T) -> BTreeSet<String> {
    match serde_json::to_value(value).expect("value should serialize") {
        Value::Object(map) => map.keys().cloned().collect(),
        other => panic!("expected an object payload, got {other}"),
    }
}

fn frozen(shape: &str) -> BTreeSet<String> {
    let raw = include_str!("../../tests/fixtures/ipc-contract.json");
    let fixture: Value = serde_json::from_str(raw).expect("the IPC fixture should be valid JSON");

    fixture["payloadShapes"][shape]
        .as_array()
        .unwrap_or_else(|| panic!("fixture has no payloadShapes.{shape}"))
        .iter()
        .map(|key| key.as_str().unwrap_or_default().to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------------

#[test]
fn timestamps_normalize_every_harness_dialect() {
    // Codex, Grok and Hermes store epoch seconds; Claude's history stores
    // millis; Antigravity stores ISO text with a nanosecond fraction.
    assert_eq!(utils::timestamp_secs("1786241643"), Some(1_786_241_643));
    assert_eq!(utils::timestamp_secs("1789947957028"), Some(1_789_947_957));
    assert_eq!(
        utils::timestamp_secs("2026-08-18 12:11:33.083102087+00:00"),
        Some(1_787_055_093)
    );
    assert_eq!(
        utils::timestamp_secs("2026-09-18T03:15:13.763Z"),
        Some(1_789_701_313)
    );

    // A positive offset is subtracted to reach UTC.
    assert_eq!(
        utils::timestamp_secs("2026-08-18T12:11:33+02:00"),
        Some(1_787_047_893)
    );

    // Unknown text is reported as-is rather than silently invented.
    assert_eq!(
        utils::normalized_timestamp("not a timestamp"),
        "not a timestamp"
    );
}

#[test]
fn iso8601_formatting_round_trips() {
    assert_eq!(utils::iso8601_from_secs(0), "1970-01-01T00:00:00Z");
    assert_eq!(
        utils::iso8601_from_secs(1_787_055_093),
        "2026-08-18T12:11:33Z"
    );

    for secs in [1_787_055_093_i64, 1_789_947_957, 951_782_400] {
        let rendered = utils::iso8601_from_secs(secs);
        assert_eq!(
            utils::timestamp_secs(&rendered),
            Some(secs),
            "{rendered} should round-trip"
        );
    }
}

// ---------------------------------------------------------------------------
// Titles
// ---------------------------------------------------------------------------

#[test]
fn titles_are_collapsed_and_character_trimmed() {
    assert_eq!(utils::trim_title("  hello\n\n  world  "), "hello world");

    let long = "a".repeat(400);
    let trimmed = utils::trim_title(&long);
    assert_eq!(trimmed.chars().count(), super::types::TITLE_LIMIT);
    assert!(trimmed.ends_with('…'));

    // Trimming counts characters, so a multi-byte title is never cut mid-glyph.
    let emoji = "🦀".repeat(400);
    let trimmed = utils::trim_title(&emoji);
    assert_eq!(trimmed.chars().count(), super::types::TITLE_LIMIT);
}

// ---------------------------------------------------------------------------
// Policy sidecar
// ---------------------------------------------------------------------------

#[test]
fn a_missing_policy_file_reads_as_defaults() {
    let root = TestRoot::new();
    let file = policy::load_from(root.path()).expect("a missing policy is not an error");

    assert!(file.pinned_sessions.is_empty());
    assert_eq!(file.retention.default_ttl_days, 15);
    assert_eq!(file.retention.auto_purge_interval_hours, 24);
    assert!(!file.retention.auto_purge_enabled);
    assert_eq!(file.retention.last_run, None);
}

#[test]
fn saving_the_policy_is_atomic_and_leaves_no_temporary_files() {
    let root = TestRoot::new();
    let file = PolicyFile {
        pinned_sessions: vec![PinnedSession {
            harness: "codex".to_string(),
            session_id: "019f7989".to_string(),
            pinned_at: "2026-10-18T05:00:00Z".to_string(),
            label: Some("Kernel audio".to_string()),
        }],
        retention: RetentionPolicy {
            default_ttl_days: 7,
            ..RetentionPolicy::default()
        },
    };

    policy::save_to(root.path(), &file).expect("policy should be written");
    let reloaded = policy::load_from(root.path()).expect("policy should be readable");

    assert_eq!(reloaded.pinned_sessions, file.pinned_sessions);
    assert_eq!(reloaded.retention.default_ttl_days, 7);

    let leftovers = fs::read_dir(root.path())
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .count();
    assert_eq!(
        leftovers, 0,
        "the atomic write must not leave a temporary file"
    );
}

#[test]
fn toggling_a_pin_keeps_retention_and_reports_the_new_state() {
    let root = TestRoot::new();
    let at = || "2026-10-18T06:00:00Z".to_string();

    assert!(
        policy::toggle_pin_at(root.path(), "codex", "abc", Some("  Pinned  ".into()), at())
            .expect("first toggle pins")
    );

    let pinned = policy::load_from(root.path()).unwrap().pinned_sessions;
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].label.as_deref(), Some("Pinned"));

    policy::save_to(
        root.path(),
        &PolicyFile {
            pinned_sessions: pinned,
            retention: RetentionPolicy {
                default_ttl_days: 30,
                ..RetentionPolicy::default()
            },
        },
    )
    .unwrap();

    assert!(
        !policy::toggle_pin_at(root.path(), "codex", "abc", None, at())
            .expect("second toggle unpins")
    );

    let file = policy::load_from(root.path()).unwrap();
    assert!(file.pinned_sessions.is_empty());
    assert_eq!(
        file.retention.default_ttl_days, 30,
        "pins must not disturb retention"
    );
}

#[test]
fn an_empty_pin_label_is_stored_as_none() {
    let root = TestRoot::new();
    policy::toggle_pin_at(
        root.path(),
        "claude",
        "s1",
        Some("   ".into()),
        "2026-10-18T06:00:00Z".to_string(),
    )
    .unwrap();

    let pinned = policy::load_from(root.path()).unwrap().pinned_sessions;
    assert_eq!(pinned[0].label, None);
}

#[test]
fn policy_values_are_clamped_and_overrides_win_over_the_default() {
    let policy = policy::sanitized(RetentionPolicy {
        default_ttl_days: 99_999,
        harness_overrides: [("codex".to_string(), 99_999)].into_iter().collect(),
        auto_purge_enabled: true,
        auto_purge_interval_hours: 0,
        last_run: None,
    });

    assert_eq!(policy.default_ttl_days, 3650);
    assert_eq!(policy.auto_purge_interval_hours, 1);
    assert_eq!(policy.ttl_days_for("codex"), 3650);
    assert_eq!(policy.ttl_days_for("claude"), 3650);
}

// ---------------------------------------------------------------------------
// Real stores
// ---------------------------------------------------------------------------

/// Read-only smoke test against the developer's own harness stores, which the
/// fixtures cannot stand in for. Ignored by default because it depends on the
/// machine; run it with
/// `cargo test --lib real_harness_smoke -- --ignored --nocapture`.
#[test]
#[ignore = "reads the harness stores of whoever runs it"]
fn real_harness_smoke() {
    let home = super::adapters::home().expect("HOME should be set");
    let mut sessions = Vec::new();

    for adapter in super::adapters::registry(&home) {
        let started = std::time::Instant::now();
        let found = adapter
            .list_sessions()
            .unwrap_or_else(|error| panic!("{} failed: {error}", adapter.harness_id()));
        eprintln!(
            "{:<12} {:>5} sessions in {:>7.1?}",
            adapter.harness_id(),
            found.len(),
            started.elapsed()
        );
        sessions.extend(found);
    }

    let bytes: u64 = sessions.iter().map(|s| s.size_bytes).sum();
    eprintln!(
        "{:<12} {:>5} sessions, {:.1} MB",
        "total",
        sessions.len(),
        bytes as f64 / 1e6
    );
    assert!(
        !sessions.is_empty(),
        "a dev machine should have at least one session"
    );
}

/// Read-only dry run against the developer's own stores: proves the selection
/// path works on real data without deleting anything. Ignored by default; run
/// it with `cargo test --lib real_purge_estimate -- --ignored --nocapture`.
#[test]
#[ignore = "reads the harness stores of whoever runs it"]
fn real_purge_estimate() {
    use super::purge;
    use super::types::{PurgeFilter, RetentionPolicy};

    let store = purge::Store::system().expect("real store");
    let policy = RetentionPolicy::default();

    // What the scheduler would consider, per harness TTL, with pins honoured.
    let selection = purge::select_by_ttl(&store, &|harness| policy.ttl_days_for(harness))
        .expect("select real candidates");
    let bytes: u64 = selection.candidates.iter().map(|c| c.size_bytes).sum();
    eprintln!(
        "retention {:>2} days: {} candidates, {:.1} MB, {} pinned, {} active",
        policy.default_ttl_days,
        selection.candidates.len(),
        bytes as f64 / 1e6,
        selection.skipped_pinned,
        selection.skipped_active
    );

    // And what the UI's dry run would report for a blanket 7-day filter.
    let estimate = purge::estimate(
        &store,
        &PurgeFilter {
            older_than_days: Some(7),
            ..PurgeFilter::default()
        },
    )
    .expect("estimate");
    eprintln!(
        ">7 days: {} sessions, {:.1} MB across {:?}",
        estimate.sessions_count,
        estimate.reclaimed_bytes as f64 / 1e6,
        estimate.affected_harnesses
    );

    assert!(
        estimate.sessions_count <= 1168,
        "the estimate cannot exceed the catalog"
    );
}

// ---------------------------------------------------------------------------
// Wire contract
// ---------------------------------------------------------------------------

#[test]
fn session_payloads_match_the_frozen_ipc_contract() {
    assert_eq!(
        keys_of(&UnifiedSession::new(
            "codex",
            "01f1",
            "Kernel audio".to_string()
        )),
        frozen("UnifiedSession")
    );
    assert_eq!(keys_of(&SessionFilter::default()), frozen("SessionFilter"));
    assert_eq!(
        keys_of(&RetentionPolicy::default()),
        frozen("RetentionPolicy")
    );
    assert_eq!(
        keys_of(&PinnedSession {
            harness: "codex".to_string(),
            session_id: "01f1".to_string(),
            pinned_at: "2026-10-18T06:00:00Z".to_string(),
            label: None,
        }),
        frozen("PinnedSession")
    );
    assert_eq!(keys_of(&PurgeReport::default()), frozen("PurgeReport"));
}
