//! Listing cases: what each adapter reports for titles, projects, turn counts
//! and byte totals, with the fixture schema matching what it queries.

use super::{adapter, TestHome};

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
