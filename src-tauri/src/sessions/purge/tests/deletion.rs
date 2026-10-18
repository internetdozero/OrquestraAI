#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Per-harness deletion: what each adapter must remove, and what it must
//! leave behind.

use std::fs;

use super::{execute, Fixture};
use crate::sessions::types::PurgeFilter;

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
// Per-harness deletion
// ---------------------------------------------------------------------------

#[test]
fn freebuff_removes_the_worktree_but_never_follows_a_link_out_of_it() {
    let fixture = Fixture::new();
    let outside = fixture.home.join("outside/precious.txt");
    fs::create_dir_all(outside.parent().unwrap()).unwrap();
    fs::write(&outside, "unrecoverable").unwrap();

    fixture.write(
        ".freebuff/worktrees/cb07e0aa/turma/turma.thread.json",
        "{\"title\":\"Turma\"}",
    );
    fixture.write(
        ".freebuff/worktrees/cb07e0aa/turma/node_modules/pkg/index.js",
        "module.exports = 1;",
    );
    // A link *inside* the worktree pointing out of it: deleting the tree must
    // unlink it, never recurse into the target.
    std::os::unix::fs::symlink(
        &outside,
        fixture
            .home
            .join(".freebuff/worktrees/cb07e0aa/turma/node_modules/link.txt"),
    )
    .unwrap();
    fixture.write(
        ".config/manicode/projects/app/chats/chat-1/chat-messages.json",
        "[{\"role\":\"user\",\"content\":\"ship it\"}]",
    );

    let report = execute(&fixture.store(), &PurgeFilter::default()).expect("purge");
    assert_eq!(report.deleted_count, 2);

    assert!(!fixture.exists(".freebuff/worktrees/cb07e0aa"));
    assert!(!fixture.exists(".config/manicode/projects/app/chats/chat-1"));
    assert!(
        outside.exists(),
        "a symlink target outside the store must survive"
    );
}

#[test]
fn claude_filters_history_and_leaves_a_project_that_still_holds_data() {
    let fixture = Fixture::new();
    fixture.write(
        ".claude/history.jsonl",
        "{\"display\":\"first\",\"timestamp\":1789947957028,\"project\":\"/home/system\",\
          \"sessionId\":\"gone\"}\n\
         {\"display\":\"other\",\"timestamp\":1789947964449,\"project\":\"/home/system\",\
          \"sessionId\":\"kept\"}\n",
    );
    fixture.write(
        ".claude/projects/-home-system/gone.jsonl",
        "{\"type\":\"user\"}\n",
    );
    fixture.write(
        ".claude/projects/-home-system/kept.jsonl",
        "{\"type\":\"user\"}\n",
    );

    execute(
        &fixture.store(),
        &PurgeFilter {
            session_ids: Some(vec!["gone".to_string()]),
            ..PurgeFilter::default()
        },
    )
    .expect("purge");

    assert!(!fixture.exists(".claude/projects/-home-system/gone.jsonl"));
    assert!(fixture.exists(".claude/projects/-home-system/kept.jsonl"));

    let history = fs::read_to_string(fixture.home.join(".claude/history.jsonl")).unwrap();
    assert!(!history.contains("\"gone\""));
    assert!(history.contains("\"kept\""));
    assert!(
        fixture.exists(".claude/projects/-home-system"),
        "a project folder holding another session stays"
    );
}

#[test]
fn claude_removes_a_project_folder_left_completely_empty() {
    let fixture = Fixture::new();
    fixture.write(
        ".claude/history.jsonl",
        "{\"display\":\"only\",\"timestamp\":1789947957028,\"sessionId\":\"only\"}\n",
    );
    fixture.write(
        ".claude/projects/-home-only/only.jsonl",
        "{\"type\":\"user\"}\n",
    );

    execute(&fixture.store(), &PurgeFilter::default()).expect("purge");

    assert!(!fixture.exists(".claude/projects/-home-only/only.jsonl"));
    assert!(!fixture.exists(".claude/projects/-home-only"));
}

#[test]
fn hermes_cascades_messages_and_removes_both_log_folders() {
    let fixture = Fixture::new();
    fixture.database(
        ".hermes/state.db",
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, source TEXT, display_name TEXT,
                                started_at REAL, ended_at REAL, message_count INTEGER);
         CREATE TABLE messages (id INTEGER PRIMARY KEY, session_id TEXT, content TEXT);
         INSERT INTO sessions VALUES ('h1', 'cli', 'Old', 1787055093.0, 1787058693.0, 1);
         INSERT INTO messages VALUES (1, 'h1', 'hello');",
    );
    fixture.write(".hermes/sessions/h1.jsonl", "abcd");
    fixture.write(".hermes/terminal-sessions/h1.jsonl", "abcd");

    let report = execute(&fixture.store(), &PurgeFilter::default()).expect("purge");

    assert_eq!(report.deleted_count, 1);
    assert_eq!(report.reclaimed_bytes, 8);
    assert_eq!(
        fixture.scalar(".hermes/state.db", "SELECT COUNT(*) FROM sessions"),
        0
    );
    assert_eq!(
        fixture.scalar(".hermes/state.db", "SELECT COUNT(*) FROM messages"),
        0,
        "messages must not be orphaned"
    );
    assert!(!fixture.exists(".hermes/sessions/h1.jsonl"));
    assert!(!fixture.exists(".hermes/terminal-sessions/h1.jsonl"));
}

#[test]
fn antigravity_removes_brain_conversation_and_summary_row() {
    let fixture = Fixture::new();
    fixture.database(
        ".gemini/antigravity/conversation_summaries.db",
        "CREATE TABLE conversation_summaries (conversation_id TEXT PRIMARY KEY, title TEXT,
            preview TEXT, step_count INTEGER, last_modified_time TEXT, workspace_uris TEXT);
         INSERT INTO conversation_summaries VALUES ('c1', 'One', '', 1,
            '2026-08-18 12:11:33.083102087+00:00', '[]');",
    );
    let brain = fixture.write(".gemini/antigravity/brain/c1/logs/transcript.jsonl", "abcd");
    let conversation = fixture.write(".gemini/antigravity/conversations/c1.db", "xyz");
    fixture.write(".gemini/antigravity/conversations/c1.db-wal", "12");

    let report = execute(&fixture.store(), &PurgeFilter::default()).expect("purge");

    assert_eq!(report.deleted_count, 1);
    assert_eq!(report.reclaimed_bytes, brain + conversation + 2);
    assert!(!fixture.exists(".gemini/antigravity/brain/c1"));
    assert!(!fixture.exists(".gemini/antigravity/conversations/c1.db"));
    assert!(!fixture.exists(".gemini/antigravity/conversations/c1.db-wal"));
    assert_eq!(
        fixture.scalar(
            ".gemini/antigravity/conversation_summaries.db",
            "SELECT COUNT(*) FROM conversation_summaries"
        ),
        0
    );
}

#[test]
fn copilot_drops_the_open_session_registry_entry() {
    let fixture = Fixture::new();
    fixture.database(
        ".copilot/session-store.db",
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, cwd TEXT, repository TEXT, summary TEXT,
                                created_at TEXT, updated_at TEXT);
         CREATE TABLE turns (id INTEGER PRIMARY KEY, session_id TEXT, user_message TEXT,
                             assistant_response TEXT);
         INSERT INTO sessions VALUES ('s1', '/work', NULL, 'Done', NULL, NULL);
         INSERT INTO turns VALUES (1, 's1', 'hi', 'there');",
    );
    fixture.write(".copilot/session-state/s1/state.json", "12345");
    fixture.write(
        ".copilot/open-sessions-state.json",
        "{\"s1\": {\"schemaVersion\": 1}, \"s2\": {\"schemaVersion\": 1}}",
    );

    execute(&fixture.store(), &PurgeFilter::default()).expect("purge");

    assert_eq!(
        fixture.scalar(".copilot/session-store.db", "SELECT COUNT(*) FROM sessions"),
        0
    );
    assert_eq!(
        fixture.scalar(".copilot/session-store.db", "SELECT COUNT(*) FROM turns"),
        0
    );
    assert!(!fixture.exists(".copilot/session-state/s1/state.json"));

    let state: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(fixture.home.join(".copilot/open-sessions-state.json")).unwrap(),
    )
    .expect("open-sessions-state stays valid JSON");
    assert!(state.get("s1").is_none(), "the stale key must be dropped");
    assert!(
        state.get("s2").is_some(),
        "other sessions must be untouched"
    );
}

#[test]
fn grok_cleans_the_search_index_and_the_session_folder() {
    let fixture = Fixture::new();
    fixture.database(
        ".grok/sessions/session_search.sqlite",
        "CREATE TABLE session_docs (session_id TEXT PRIMARY KEY, cwd TEXT, updated_at INTEGER,
                                    title TEXT);
         INSERT INTO session_docs VALUES ('a1', '/work', 1791708904, 'Old');
         INSERT INTO session_docs VALUES ('a2', '/work', 1791708904, 'Keep');",
    );
    fixture.write(".grok/sessions/-work/a1/state.json", "123456");
    fixture.write(".grok/sessions/-work/a2/state.json", "123456");

    execute(
        &fixture.store(),
        &PurgeFilter {
            session_ids: Some(vec!["a1".to_string()]),
            ..PurgeFilter::default()
        },
    )
    .expect("purge");

    assert!(!fixture.exists(".grok/sessions/-work/a1"));
    assert!(fixture.exists(".grok/sessions/-work/a2"));
    assert_eq!(
        fixture.scalar(
            ".grok/sessions/session_search.sqlite",
            "SELECT COUNT(*) FROM session_docs"
        ),
        1,
        "the index must not keep an orphan document"
    );
}
