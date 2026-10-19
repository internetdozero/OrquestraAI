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

#[test]
fn freebuff_measures_worktree_bytes_and_chat_turns() {
    let home = TestHome::new();
    let meta = home.write(
        ".freebuff/worktrees/cb07e0aa/turma/turma.thread.json",
        "{\"title\":\"Turma migration\",\"cwd\":\"/work/turma\"}",
    );
    // The reason worktrees matter: a whole node_modules per thread.
    let modules = home.write(
        ".freebuff/worktrees/cb07e0aa/turma/node_modules/pkg/index.js",
        "module.exports = 1;",
    );
    home.write(
        ".config/manicode/projects/app/chats/chat-1/chat-messages.json",
        "[{\"role\":\"user\",\"content\":\"ship it\"},{\"role\":\"assistant\",\"content\":\"ok\"},\
          {\"role\":\"user\",\"content\":\"again\"}]",
    );

    let sessions = adapter(&home.0, "freebuff").list_sessions().expect("list");
    assert_eq!(sessions.len(), 2);

    let worktree = sessions
        .iter()
        .find(|s| s.id == "cb07e0aa")
        .expect("worktree");
    assert_eq!(worktree.title, "Turma migration");
    assert_eq!(worktree.project_path.as_deref(), Some("/work/turma"));
    assert_eq!(worktree.size_bytes, meta + modules);

    let chat = sessions
        .iter()
        .find(|s| s.id == "app/chat-1")
        .expect("chat");
    assert_eq!(chat.title, "ship it");
    assert_eq!(chat.turns_count, 2);
}

#[test]
fn freebuff_desktop_lists_threads_and_deletes_them() {
    let home = TestHome::new();
    let thread = "11111111-2222-3333-4444-555555555555";

    home.write(
        ".config/freebuff-desktop/projects/OrquestraAI-abc123/project.json",
        "{\"projectId\":\"abc123\",\"projectPath\":\"/work/orquestra\",\"database\":\"desktop-v2.db\"}",
    );
    home.database(
        ".config/freebuff-desktop/projects/OrquestraAI-abc123/desktop-v2.db",
        "CREATE TABLE threads (id TEXT PRIMARY KEY, project_id TEXT, project_path TEXT,
                                title TEXT, created_at INTEGER, updated_at INTEGER);
         CREATE TABLE messages (seq INTEGER PRIMARY KEY AUTOINCREMENT, thread_id TEXT,
                                role TEXT, parts_json TEXT DEFAULT '[]',
                                attachments_json TEXT DEFAULT '[]', ts INTEGER);
         CREATE TABLE freebuff_conversation_search (text TEXT, thread_id TEXT);
         INSERT INTO threads VALUES ('11111111-2222-3333-4444-555555555555', 'abc123',
           '/work/stale', 'Desktop native', 1792324543519, 1792324942904);
         INSERT INTO messages (thread_id, role, parts_json, attachments_json, ts) VALUES
           ('11111111-2222-3333-4444-555555555555', 'user', '[{\"kind\":\"text\"}]', '[]', 1792324544000),
           ('11111111-2222-3333-4444-555555555555', 'assistant', '[{\"kind\":\"text\"}]', '[]', 1792324600000);
         INSERT INTO freebuff_conversation_search VALUES
           ('hello', '11111111-2222-3333-4444-555555555555');",
    );
    // A project without project.json falls back to the row's project_path.
    home.database(
        ".config/freebuff-desktop/projects/plain-xyz/desktop-v2.db",
        "CREATE TABLE threads (id TEXT PRIMARY KEY, project_id TEXT, project_path TEXT,
                                title TEXT, created_at INTEGER, updated_at INTEGER);
         INSERT INTO threads VALUES ('22222222-2222-3333-4444-555555555555', 'xyz',
           '/work/fallback', '', 1792324543519, 1792324942904);",
    );
    let folder = home.write(
        &format!(".config/freebuff-desktop/local-projects/abc123/threads/{thread}/state.json"),
        "12345",
    );
    let tree = home.write(
        &format!(".freebuff/worktrees/{thread}/node_modules/pkg/index.js"),
        "module.exports = 1;",
    );

    let sessions = adapter(&home.0, "freebuff").list_sessions().expect("list");
    let desktop = sessions
        .iter()
        .find(|s| s.id == format!("desktop:OrquestraAI-abc123:{thread}"))
        .expect("desktop thread");
    assert_eq!(desktop.title, "Desktop native");
    // project.json wins over the stale row path.
    assert_eq!(desktop.project_path.as_deref(), Some("/work/orquestra"));
    assert_eq!(desktop.turns_count, 2);
    // Epoch millis normalized to ISO 8601 UTC.
    assert_eq!(desktop.updated_at, "2026-10-18T12:02:22Z");
    assert_eq!(desktop.created_at.as_deref(), Some("2026-10-18T11:55:43Z"));
    // Message payload bytes plus the thread folder plus the worktree.
    let payloads = "[{\"kind\":\"text\"}]".len() as u64 * 2 + "[]".len() as u64 * 2;
    assert_eq!(desktop.size_bytes, payloads + folder + tree);

    let fallback = sessions
        .iter()
        .find(|s| s.id == "desktop:plain-xyz:22222222-2222-3333-4444-555555555555")
        .expect("fallback thread");
    assert_eq!(fallback.project_path.as_deref(), Some("/work/fallback"));
    assert_eq!(fallback.title, "Thread 22222222");
    assert_eq!(fallback.turns_count, 0, "no messages table, no turns");

    let freed = adapter(&home.0, "freebuff")
        .delete_session(&desktop.id)
        .expect("delete");
    assert_eq!(freed, folder + tree);
    assert!(
        !home.0.join(format!(".freebuff/worktrees/{thread}")).exists(),
        "the same-id worktree is reclaimed"
    );

    let remaining: i64 = Connection::open(home.0.join(
        ".config/freebuff-desktop/projects/OrquestraAI-abc123/desktop-v2.db",
    ))
    .expect("reopen")
    .query_row(
        "SELECT (SELECT COUNT(*) FROM threads) + (SELECT COUNT(*) FROM messages) + \
         (SELECT COUNT(*) FROM freebuff_conversation_search)",
        [],
        |row| row.get(0),
    )
    .expect("count");
    assert_eq!(remaining, 0, "thread, messages and FTS rows are gone");

    // VACUUM over the desktop databases must succeed after the delete.
    adapter(&home.0, "freebuff")
        .optimize_storage()
        .expect("vacuum");

    // Only the untouched project remains: the same-id worktree was reclaimed
    // too, so its worktree session is gone as well.
    let relisted = adapter(&home.0, "freebuff")
        .list_sessions()
        .expect("relist");
    assert_eq!(sessions.len(), 2, "thread + fallback; same-id worktree is deduped");
    assert_eq!(
        relisted.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        [fallback.id.as_str()]
    );
}

#[test]
fn freebuff_desktop_ids_reject_traversal_and_malformed_shapes() {
    use super::freebuff_desktop::parse_id;

    assert!(parse_id("desktop:OrquestraAI-abc:11111111-2222-3333-4444-555555555555").is_some());
    for hostile in [
        "desktop:..:11111111",
        "desktop:.:11111111",
        "desktop:slug:..",
        "desktop:slug:.",
        "desktop:../escape:11111111",
        "desktop:slug:../escape",
        "desktop:slug:extra:segment",
        "desktop:nosplit",
        "desktop::empty",
        "desktop:slug:",
        "cb07e0aa",
        "app/chat-1",
        "",
    ] {
        assert!(
            parse_id(hostile).is_none(),
            "{hostile:?} must not parse as a desktop thread"
        );
    }
}

#[test]
fn hermes_counts_messages_and_log_files() {
    let home = TestHome::new();
    home.database(
        ".hermes/state.db",
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, source TEXT, display_name TEXT,
                                started_at REAL, ended_at REAL, message_count INTEGER);
         CREATE TABLE messages (id INTEGER PRIMARY KEY, session_id TEXT, content TEXT);
         INSERT INTO sessions VALUES ('h1', 'cli', 'Ops triage', 1787055093.0, 1787058693.0, 3);
         INSERT INTO messages VALUES (1, 'h1', 'hello');",
    );
    let log = home.write(".hermes/sessions/h1.jsonl", "abcd");

    let sessions = adapter(&home.0, "hermes").list_sessions().expect("list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Ops triage");
    assert_eq!(sessions[0].turns_count, 3, "message_count is authoritative");
    assert_eq!(sessions[0].size_bytes, 5 + log);
    assert_eq!(
        sessions[0].created_at.as_deref(),
        Some("2026-08-18T12:11:33Z")
    );
    assert_eq!(sessions[0].updated_at, "2026-08-18T13:11:33Z");
}

#[test]
fn grok_merges_the_index_with_session_folders() {
    let home = TestHome::new();
    home.database(
        ".grok/sessions/session_search.sqlite",
        "CREATE TABLE session_docs (session_id TEXT PRIMARY KEY, cwd TEXT, updated_at INTEGER,
                                    title TEXT);
         INSERT INTO session_docs VALUES ('01a128d1', '/work/cuida-te', 1791708904, 'Redesign');",
    );
    let indexed = home.write(
        ".grok/sessions/-work-cuida-te/01a128d1/state.json",
        "123456",
    );
    let unindexed = home.write(".grok/sessions/%2Fwork%2Fb/01a1396b/log.jsonl", "12");

    let mut sessions = adapter(&home.0, "grok").list_sessions().expect("list");
    sessions.sort_by(|left, right| left.id.cmp(&right.id));

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].title, "Redesign");
    assert_eq!(sessions[0].project_path.as_deref(), Some("/work/cuida-te"));
    assert_eq!(sessions[0].size_bytes, indexed);
    assert_eq!(sessions[0].updated_at, "2026-10-11T08:55:04Z");

    // Only the folder exists, so the encoded path is decoded for the project.
    assert_eq!(sessions[1].project_path.as_deref(), Some("/work/b"));
    assert_eq!(sessions[1].size_bytes, unindexed);
}

#[test]
fn a_corrupt_harness_database_is_skipped_without_losing_the_others() {
    let home = TestHome::new();
    // Not a database at all: SQLite will refuse to read it.
    home.write(
        ".gemini/antigravity/conversation_summaries.db",
        "definitely not sqlite",
    );
    home.database(
        ".local/share/opencode/opencode.db",
        "CREATE TABLE session (id TEXT, title TEXT, directory TEXT, time_updated INTEGER);
         INSERT INTO session VALUES ('ses_1', 'Still here', '/work', 1789403075184);",
    );

    let (sessions, warnings) = super::collect(&home.0);

    assert_eq!(sessions.len(), 1, "the healthy harness must survive");
    assert_eq!(sessions[0].title, "Still here");
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].starts_with("antigravity:"),
        "the warning should name the failing harness, got {:?}",
        warnings[0]
    );
}

#[test]
fn freebuff_dedupes_prefix_worktrees_against_desktop_threads() {
    let home = TestHome::new();
    let thread = "cb07e0aa-1111-2222-3333-444444444444";
    let slug = "Proj-abc";

    home.write(
        &format!(".config/freebuff-desktop/projects/{slug}/project.json"),
        "{\"projectId\":\"abc\",\"projectPath\":\"/work/proj\"}",
    );
    home.database(
        &format!(".config/freebuff-desktop/projects/{slug}/desktop-v2.db"),
        &format!(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, project_id TEXT, project_path TEXT,\
                                title TEXT, created_at INTEGER, updated_at INTEGER);\
             CREATE TABLE messages (seq INTEGER PRIMARY KEY AUTOINCREMENT, thread_id TEXT,\
                                role TEXT, parts_json TEXT DEFAULT '[]',\
                                attachments_json TEXT DEFAULT '[]', ts INTEGER);\
             INSERT INTO threads VALUES ('{thread}', 'abc', '/work/proj',\
               'Prefix thread', 1792324543519, 1792324942904);\
             INSERT INTO messages (thread_id, role, parts_json, attachments_json, ts) VALUES\
               ('{thread}', 'user', '[12345]', '[]', 1792324544000);"
        ),
    );
    let claimed = home.write(".freebuff/worktrees/cb07e0aa/payload.bin", "claimed-bytes");
    let orphan = home.write(".freebuff/worktrees/deadbeef/payload.bin", "orphan-bytes!");

    let sessions = adapter(&home.0, "freebuff").list_sessions().expect("list");
    let desktop = sessions
        .iter()
        .find(|s| s.id == format!("desktop:{slug}:{thread}"))
        .expect("desktop thread");
    // Message payload (7) + attachments (2) plus the prefix worktree bytes.
    assert_eq!(desktop.size_bytes, 7 + 2 + claimed);
    assert!(
        sessions.iter().all(|s| s.id != "cb07e0aa"),
        "prefix worktree must not appear as a duplicate bare session"
    );
    assert!(
        sessions.iter().any(|s| s.id == "deadbeef"),
        "unassociated orphan worktrees stay listed for purging"
    );
    let orphan_session = sessions.iter().find(|s| s.id == "deadbeef").unwrap();
    assert_eq!(orphan_session.size_bytes, orphan);

    let freed = adapter(&home.0, "freebuff")
        .delete_session(&desktop.id)
        .expect("delete");
    assert_eq!(freed, claimed, "prefix worktree bytes are reclaimed");
    assert!(
        !home.0.join(".freebuff/worktrees/cb07e0aa").exists(),
        "the prefix worktree is unlinked"
    );
    assert!(
        home.0.join(".freebuff/worktrees/deadbeef").exists(),
        "the orphan worktree is untouched"
    );

    let relisted = adapter(&home.0, "freebuff").list_sessions().expect("relist");
    assert!(
        relisted.iter().any(|s| s.id == "deadbeef"),
        "orphan survives the desktop delete"
    );
    assert!(
        relisted.iter().all(|s| s.id != "cb07e0aa"),
        "no ghost card returns for the reclaimed prefix"
    );
}

#[test]
fn an_absent_harness_is_silently_skipped() {
    let home = TestHome::new();
    let (sessions, warnings) = super::collect(&home.0);

    assert!(sessions.is_empty());
    assert!(
        warnings.is_empty(),
        "a missing harness is not an error: {warnings:?}"
    );
    for harness in super::HARNESS_IDS {
        assert!(
            !adapter(&home.0, harness).is_available(),
            "{harness} is not installed"
        );
    }
}
