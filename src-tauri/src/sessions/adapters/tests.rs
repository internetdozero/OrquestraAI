#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Adapter discovery tests.
//!
//! Every fixture lives under `/var/tmp/testing` and mimics the real store
//! layout — including the columns the adapters actually query, so a schema
//! drift in this file means the query drifted too.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{registry, SessionAdapter};

mod stores;

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

#[test]
fn claude_transcript_reads_turns_and_skips_noise() {
    let home = TestHome::new();
    home.write(
        ".claude/projects/-work-app/abc123.jsonl",
        "{\"type\":\"mode\",\"sessionId\":\"abc123\"}\n\
         {\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\
          [{\"type\":\"text\",\"text\":\"first prompt\"}]},\
          \"timestamp\":\"2026-09-20T23:46:04.449Z\"}\n\
         this line is not json\n\
         {\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\
          [{\"type\":\"tool_result\",\"tool_use_id\":\"t1\",\"content\":\"huge dump\"}]}}\n\
         {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":\
          [{\"type\":\"text\",\"text\":\"here you go\"},\
           {\"type\":\"tool_use\",\"name\":\"Bash\",\"input\":{}}]},\
          \"timestamp\":\"2026-09-20T23:46:10.000Z\"}\n\
         {\"type\":\"summary\",\"summary\":\"compacted\"}\n",
    );

    let turns = adapter(&home.0, "claude").read_transcript("abc123");
    assert_eq!(turns.len(), 2, "mode, corrupt, tool-only and summary lines are skipped");
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "first prompt");
    assert_eq!(
        turns[0].timestamp.as_deref(),
        Some("2026-09-20T23:46:04Z")
    );
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "here you go");

    assert!(
        adapter(&home.0, "claude").read_transcript("missing").is_empty(),
        "no transcript file is an empty feed, not an error"
    );
}

#[test]
fn antigravity_transcript_covers_both_log_layouts() {
    let home = TestHome::new();
    home.write(
        ".gemini/antigravity/brain/a8ceb54b/logs/transcript.jsonl",
        "{\"role\":\"user\",\"content\":\"fix the kernel\",\
          \"timestamp\":\"2026-08-18T12:11:33Z\"}\n\
         {\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"on it\"}]}\n\
         {\"unrelated\":\"shape\"}\n",
    );
    home.write(
        ".gemini/antigravity-cli/brain/c1/.system_generated/logs/transcript.jsonl",
        "{\"role\":\"user\",\"content\":\"cli hello\"}\n",
    );

    let turns = adapter(&home.0, "antigravity").read_transcript("a8ceb54b");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "fix the kernel");
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "on it");

    let cli = adapter(&home.0, "antigravity").read_transcript("c1");
    assert_eq!(cli.len(), 1);
    assert_eq!(cli[0].content, "cli hello");

    assert!(
        adapter(&home.0, "antigravity").read_transcript("missing").is_empty()
    );
}

#[test]
fn harnesses_without_data_report_an_empty_feed() {
    // Every harness has a transcript reader now; with no store on disk each
    // one must report an empty feed rather than an error. Readers with
    // fixture data are covered in `transcript_tests`.
    let home = TestHome::new();
    for harness in super::HARNESS_IDS {
        assert!(
            adapter(&home.0, harness).read_transcript("whatever").is_empty(),
            "{harness} must return an empty feed when its store is missing"
        );
    }
}

#[test]
fn opencode_and_kilo_report_titles_projects_and_artifact_bytes() {
    let home = TestHome::new();
    for store in ["opencode", "kilo"] {
        home.database(
            &format!(".local/share/{store}/{store}.db"),
            "CREATE TABLE session (id TEXT, title TEXT, directory TEXT,
                                   time_updated INTEGER, time_created INTEGER);
             INSERT INTO session VALUES
               ('ses_1', 'Refactor parser', '/work/analyzer', 1789403075184, 1789400000000);",
        );
    }
    let artifact = home.write(
        ".local/share/opencode/storage/session_diff/ses_1/part.json",
        "{\"diff\":\"x\"}",
    );

    for store in ["opencode", "kilo"] {
        let sessions = adapter(&home.0, store).list_sessions().expect("list");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title, "Refactor parser");
        assert_eq!(sessions[0].project_path.as_deref(), Some("/work/analyzer"));
        // Millis from `time_updated`, normalized to ISO 8601 UTC.
        assert_eq!(sessions[0].updated_at, "2026-09-14T16:24:35Z");
    }

    // Only OpenCode has the artifact folder in this fixture.
    assert_eq!(
        adapter(&home.0, "opencode").list_sessions().unwrap()[0].size_bytes,
        artifact
    );
    assert_eq!(
        adapter(&home.0, "kilo").list_sessions().unwrap()[0].size_bytes,
        0
    );
}

#[test]
fn copilot_merges_turn_counts_state_folders_and_orphans() {
    let home = TestHome::new();
    home.database(
        ".copilot/session-store.db",
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, cwd TEXT, repository TEXT,
                                summary TEXT, created_at TEXT, updated_at TEXT);
         CREATE TABLE turns (id INTEGER PRIMARY KEY, session_id TEXT, turn_index INTEGER,
                             user_message TEXT, assistant_response TEXT, timestamp TEXT);
         INSERT INTO sessions VALUES ('s1', '/work/app', 'app', 'Check Recent Pushes',
             '2026-09-18T03:15:13.763Z', '2026-09-18T07:06:11.731Z');
         INSERT INTO turns VALUES (1, 's1', 0, 'abcd', '', '2026-09-18T03:16:00Z');
         INSERT INTO turns VALUES (2, 's1', 1, '', 'xyz', '2026-09-18T03:17:00Z');",
    );
    let state = home.write(".copilot/session-state/s1/state.json", "12345");
    home.write(".copilot/session-state/orphan/state.json", "7");

    let sessions = adapter(&home.0, "copilot").list_sessions().expect("list");
    assert_eq!(
        sessions.len(),
        2,
        "the orphan state folder is still on disk"
    );

    let stored = sessions
        .iter()
        .find(|s| s.id == "s1")
        .expect("indexed session");
    assert_eq!(stored.title, "Check Recent Pushes");
    assert_eq!(stored.turns_count, 2);
    // Turn content bytes (7) plus the state folder (5).
    assert_eq!(stored.size_bytes, 7 + state);
    assert_eq!(stored.updated_at, "2026-09-18T07:06:11Z");

    let orphan = sessions
        .iter()
        .find(|s| s.id == "orphan")
        .expect("orphan session");
    assert_eq!(orphan.title, "Session orphan");
}

#[test]
fn antigravity_reads_titles_step_counts_and_workspace_uris() {
    let home = TestHome::new();
    home.database(
        ".gemini/antigravity/conversation_summaries.db",
        "CREATE TABLE conversation_summaries (conversation_id TEXT PRIMARY KEY, title TEXT,
            preview TEXT, step_count INTEGER, last_modified_time TEXT, workspace_uris TEXT);
         INSERT INTO conversation_summaries VALUES
           ('a8ceb54b', '', 'Fix the audio kernel', 12,
            '2026-08-18 12:11:33.083102087+00:00',
            '[\"file:///home/system/Coding/Biblioteca/\"]');",
    );
    let brain = home.write(
        ".gemini/antigravity/brain/a8ceb54b/logs/transcript.jsonl",
        "abcd",
    );
    let conversation = home.write(".gemini/antigravity/conversations/a8ceb54b.db", "xyz");

    let sessions = adapter(&home.0, "antigravity")
        .list_sessions()
        .expect("list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "a8ceb54b");
    assert_eq!(sessions[0].title, "Fix the audio kernel");
    assert_eq!(sessions[0].turns_count, 12);
    assert_eq!(
        sessions[0].project_path.as_deref(),
        Some("/home/system/Coding/Biblioteca")
    );
    assert_eq!(sessions[0].updated_at, "2026-08-18T12:11:33Z");
    assert_eq!(sessions[0].size_bytes, brain + conversation);
}

#[test]
fn codex_unions_history_index_and_rollouts() {
    let home = TestHome::new();
    home.database(
        ".codex/thread_history_1.sqlite",
        "CREATE TABLE thread_turns (thread_id TEXT, turn_id TEXT, started_at INTEGER,
                                    completed_at INTEGER);
         INSERT INTO thread_turns VALUES ('01a148f4-9277-7820-8e2c-30f21ca5100f', 't1',
             1786241643, 1786241646);
         INSERT INTO thread_turns VALUES ('01a148f4-9277-7820-8e2c-30f21ca5100f', 't2',
             1786241667, 1786241933);",
    );
    home.write(
        ".codex/session_index.jsonl",
        "{\"id\":\"01a148f4-9277-7820-8e2c-30f21ca5100f\",\"thread_name\":\"Responder saudação\",\
         \"updated_at\":\"2026-07-19T08:41:50.320348334Z\"}\n",
    );
    let rollout = home.write(
        ".codex/sessions/2026/10/17/rollout-2026-10-17T05-22-30-01a148f4-9277-7820-8e2c-30f21ca5100f.jsonl",
        "{\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/work/analyzer\"}}\n",
    );
    // A rollout with no history rows still has bytes worth reporting.
    home.write(
        ".codex/sessions/2026/10/18/rollout-2026-10-18T05-22-30-01a15777-0000-0000-0000-000000000000.jsonl",
        "{\"type\":\"session_meta\",\"payload\":{}}\n",
    );

    let sessions = adapter(&home.0, "codex").list_sessions().expect("list");
    assert_eq!(
        sessions.len(),
        2,
        "rollouts without history rows still count"
    );

    let named = sessions
        .iter()
        .find(|s| s.id == "01a148f4-9277-7820-8e2c-30f21ca5100f")
        .expect("indexed thread");
    assert_eq!(named.title, "Responder saudação");
    assert_eq!(named.turns_count, 2);
    assert_eq!(named.project_path.as_deref(), Some("/work/analyzer"));
    assert_eq!(named.size_bytes, rollout);
    assert_eq!(named.created_at.as_deref(), Some("2026-08-09T02:14:03Z"));
    assert_eq!(named.updated_at, "2026-08-09T02:18:53Z");
}

#[test]
fn claude_prefers_history_titles_and_transcript_cwd() {
    let home = TestHome::new();
    home.write(
        ".claude/history.jsonl",
        "{\"display\":\"/usage\",\"timestamp\":1789947957028,\"project\":\"/home/system\",\
          \"sessionId\":\"153a4174\"}\n\
         {\"display\":\"oi\",\"timestamp\":1789947964449,\"project\":\"/home/system\",\
          \"sessionId\":\"153a4174\"}\n",
    );
    let transcript = home.write(
        ".claude/projects/-home-system/153a4174.jsonl",
        "{\"type\":\"mode\",\"sessionId\":\"153a4174\"}\n\
         {\"type\":\"user\",\"cwd\":\"/home/system/Persistente\",\
          \"message\":{\"content\":[{\"type\":\"text\",\"text\":\"first prompt\"}]}}\n",
    );

    let sessions = adapter(&home.0, "claude").list_sessions().expect("list");
    assert_eq!(sessions.len(), 1);
    // The slash command is skipped in favour of the first real prompt.
    assert_eq!(sessions[0].title, "oi");
    assert_eq!(sessions[0].project_path.as_deref(), Some("/home/system"));
    assert_eq!(sessions[0].turns_count, 2);
    assert_eq!(sessions[0].size_bytes, transcript);
    assert_eq!(sessions[0].updated_at, "2026-09-20T23:46:04Z");
}

