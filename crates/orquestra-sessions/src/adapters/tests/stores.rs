//! Adapter tests for the stores that keep their own on-disk layout.
//!
//! Split from the discovery module so both files stay under the line cap.

use std::fs;

use rusqlite::Connection;

use super::{adapter, TestHome};

#[test]
fn freebuff_measures_worktree_bytes_and_chat_turns() {
    let home = TestHome::new();
    let meta = home.write(
        ".freebuff/worktrees/cb07e0aa/turma/turma.thread.json",
        "{\"title\":\"Turma migration\",\"cwd\":\"/work/turma\"}",
    );
    // A whole node_modules per thread rides along in worktrees, but sizing
    // prunes dependency directories: it must not inflate the total.
    home.write(
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
    assert_eq!(worktree.size_bytes, meta, "node_modules is pruned");

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
    // Dependency ballast rides along in the worktree but stays out of every
    // byte count below: listing and reclaim report the same pruned total.
    home.write(
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
    // Message payload bytes plus the thread folder plus the worktree's real
    // files; node_modules inside the worktree is pruned.
    let payloads = "[{\"kind\":\"text\"}]".len() as u64 * 2 + "[]".len() as u64 * 2;
    assert_eq!(desktop.size_bytes, payloads + folder);

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
    assert_eq!(freed, folder, "reclaim reports the same pruned total");
    assert!(
        !home
            .0
            .join(format!(".freebuff/worktrees/{thread}"))
            .exists(),
        "the same-id worktree is reclaimed"
    );

    let remaining: i64 = Connection::open(
        home.0
            .join(".config/freebuff-desktop/projects/OrquestraAI-abc123/desktop-v2.db"),
    )
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
    assert_eq!(
        sessions.len(),
        2,
        "thread + fallback; same-id worktree is deduped"
    );
    assert_eq!(
        relisted.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        [fallback.id.as_str()]
    );
}

#[test]
fn freebuff_desktop_ids_reject_traversal_and_malformed_shapes() {
    use super::super::freebuff_desktop::parse_id;

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

    let (sessions, warnings) = super::super::collect(&home.0);

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

    let relisted = adapter(&home.0, "freebuff")
        .list_sessions()
        .expect("relist");
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
    let (sessions, warnings) = super::super::collect(&home.0);

    assert!(sessions.is_empty());
    assert!(
        warnings.is_empty(),
        "a missing harness is not an error: {warnings:?}"
    );
    for harness in super::super::HARNESS_IDS {
        assert!(
            !adapter(&home.0, harness).is_available(),
            "{harness} is not installed"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_session_file_is_not_sized_or_loaded() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let scratch = std::path::PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-file-size-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&scratch).expect("scratch");
    let outside = scratch.join("outside.jsonl");
    let link = scratch.join("session.jsonl");
    let real = scratch.join("real.jsonl");
    let line = "{\"role\":\"user\",\"content\":\"secret-outside\"}\n";
    fs::write(&outside, line).expect("outside jsonl");
    fs::write(&real, "{\"role\":\"user\",\"content\":\"kept\"}\n").expect("real jsonl");
    std::os::unix::fs::symlink(&outside, &link).expect("file symlink");

    assert_eq!(super::super::utils::file_size(&link), 0);
    assert_eq!(
        crate::adapters::fsops::size_of(&link),
        0,
        "reclaim estimates must not follow a file symlink"
    );
    assert!(super::super::utils::file_size(&real) > 0);
    assert!(
        super::super::utils::read_jsonl(&link).is_empty(),
        "the size ceiling must not treat a symlink as an empty file and then read it"
    );
    assert_eq!(super::super::utils::read_jsonl(&real)[0]["content"], "kept");
    assert!(super::super::transcript::read_transcript(&link).is_empty());
    assert_eq!(
        super::super::transcript::read_transcript(&real)[0].content,
        "kept"
    );
    assert!(outside.is_file(), "the target stays put");

    fs::remove_dir_all(&scratch).expect("cleanup");
}
