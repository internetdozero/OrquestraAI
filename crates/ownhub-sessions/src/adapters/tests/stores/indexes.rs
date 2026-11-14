//! Harnesses whose store is an index database: Hermes and Grok, plus the
//! fleet-level cases (one corrupt store, one absent harness).

use crate::adapters::{collect, HARNESS_IDS};

use super::{adapter, TestHome};

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
fn grok_ignores_and_prunes_empty_stubs_but_keeps_live_and_real_sessions() {
    let home = TestHome::new();
    home.database(
        ".grok/sessions/session_search.sqlite",
        "CREATE TABLE session_docs (session_id TEXT PRIMARY KEY, cwd TEXT, updated_at INTEGER,
                                    title TEXT);
         INSERT INTO session_docs VALUES ('indexed', '/work/indexed', 0, 'Indexed');",
    );
    std::fs::create_dir_all(home.0.join(".grok/sessions/proj/ghost/terminal")).unwrap();
    std::fs::create_dir_all(home.0.join(".grok/sessions/proj/active/terminal")).unwrap();
    std::fs::create_dir_all(home.0.join(".grok/sessions/proj/active-session/terminal")).unwrap();
    std::fs::create_dir_all(home.0.join(".grok/sessions/proj/active-id/terminal")).unwrap();
    std::fs::create_dir_all(home.0.join(".grok/sessions/proj/indexed/terminal")).unwrap();
    home.write(
        ".grok/active_sessions.json",
        "[\"active\",{\"session_id\":\"active-session\"},{\"id\":\"active-id\"}]",
    );
    home.write(
        ".grok/sessions/proj/real/chat_history.jsonl",
        "{\"type\":\"user\",\"content\":\"question\"}\n\
         {\"type\":\"assistant\",\"content\":\"answer\"}\n",
    );

    let grok = adapter(&home.0, "grok");
    let sessions = grok.list_sessions().expect("list");
    assert!(!sessions.iter().any(|session| session.id == "ghost"));
    assert!(sessions.iter().any(|session| session.id == "active"));
    assert!(sessions
        .iter()
        .any(|session| session.id == "active-session"));
    assert!(sessions.iter().any(|session| session.id == "active-id"));
    assert!(sessions.iter().any(|session| session.id == "indexed"));
    assert_eq!(
        sessions
            .iter()
            .find(|session| session.id == "real")
            .expect("real session")
            .turns_count,
        1,
        "turns_count represents parsed user turns from chat_history.jsonl"
    );

    grok.optimize_storage().expect("prune empty startup stubs");
    assert!(!home.0.join(".grok/sessions/proj/ghost").exists());
    assert!(home.0.join(".grok/sessions/proj/active/terminal").is_dir());
    assert!(home
        .0
        .join(".grok/sessions/proj/active-session/terminal")
        .is_dir());
    assert!(home
        .0
        .join(".grok/sessions/proj/active-id/terminal")
        .is_dir());
    assert!(home.0.join(".grok/sessions/proj/indexed/terminal").is_dir());
    assert!(home
        .0
        .join(".grok/sessions/proj/real/chat_history.jsonl")
        .is_file());
}

#[test]
fn grok_preserves_active_sessions_from_object_key_maps() {
    let home = TestHome::new();
    std::fs::create_dir_all(home.0.join(".grok/sessions/proj/map-active/terminal")).unwrap();
    home.write(".grok/active_sessions.json", "{\"map-active\":true}");

    let grok = adapter(&home.0, "grok");
    let sessions = grok.list_sessions().expect("list");
    assert!(sessions.iter().any(|session| session.id == "map-active"));

    grok.optimize_storage().expect("preserve active map entry");
    assert!(home
        .0
        .join(".grok/sessions/proj/map-active/terminal")
        .is_dir());
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

    let (sessions, warnings) = collect(&home.0);

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
fn an_absent_harness_is_silently_skipped() {
    let home = TestHome::new();
    let (sessions, warnings) = collect(&home.0);

    assert!(sessions.is_empty());
    assert!(
        warnings.is_empty(),
        "a missing harness is not an error: {warnings:?}"
    );
    for harness in HARNESS_IDS {
        assert!(
            !adapter(&home.0, harness).is_available(),
            "{harness} is not installed"
        );
    }
}
