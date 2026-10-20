//! Session ids that climb out of a real store directory with `..`.

use std::fs;

use super::{adapter, TestHome};

#[cfg(unix)]
#[test]
fn antigravity_read_does_not_escape_brain_with_parent_dirs() {
    let home = TestHome::new();
    let outside = home.0.join("outside-escape");
    fs::create_dir_all(outside.join("logs")).unwrap();
    fs::write(
        outside.join("logs/transcript.jsonl"),
        "{\"source\":\"USER_EXPLICIT\",\"type\":\"USER_INPUT\",\
          \"content\":\"<USER_REQUEST>ssh-private</USER_REQUEST>\"}\n",
    )
    .unwrap();
    fs::create_dir_all(home.0.join(".gemini/antigravity/brain")).unwrap();

    let turns = adapter(&home.0, "antigravity").read_transcript("../../../outside-escape");
    assert!(
        turns.iter().all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript climbed out of brain/: {turns:?}"
    );
}

#[cfg(unix)]
#[test]
fn claude_read_does_not_escape_a_project_with_parent_dirs() {
    let home = TestHome::new();
    fs::create_dir_all(home.0.join(".claude/projects/slug")).unwrap();
    fs::write(
        home.0.join("secret.jsonl"),
        "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ssh-private\"}]}}\n",
    )
    .unwrap();

    let turns = adapter(&home.0, "claude").read_transcript("../../../secret");
    assert!(
        turns.iter().all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript climbed out of projects/: {turns:?}"
    );
}

#[cfg(unix)]
#[test]
fn hermes_list_does_not_size_a_directory_outside_sessions() {
    let home = TestHome::new();
    fs::create_dir_all(home.0.join(".hermes/sessions")).unwrap();
    let outside = home.0.join("outside-hermes");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.bin"), vec![7u8; 4096]).unwrap();
    home.database(
        ".hermes/state.db",
        "CREATE TABLE sessions (
            id TEXT, source TEXT, display_name TEXT,
            started_at TEXT, ended_at TEXT, message_count INTEGER
         );
         INSERT INTO sessions VALUES (
            '../../outside-hermes', 'cli', 'x', NULL, NULL, 0
         );",
    );

    let sessions = adapter(&home.0, "hermes").list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| session.size_bytes == 0),
        "listing sized a directory outside sessions/: {sessions:?}"
    );
}

#[cfg(unix)]
#[test]
fn copilot_list_does_not_size_a_directory_outside_session_state() {
    let home = TestHome::new();
    fs::create_dir_all(home.0.join(".copilot/session-state")).unwrap();
    let outside = home.0.join("outside-copilot");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.bin"), vec![7u8; 4096]).unwrap();
    home.database(
        ".copilot/session-store.db",
        "CREATE TABLE sessions (
            id TEXT, cwd TEXT, repository TEXT, summary TEXT,
            created_at TEXT, updated_at TEXT
         );
         INSERT INTO sessions VALUES (
            '../../outside-copilot', NULL, NULL, 'x', NULL, NULL
         );",
    );

    let sessions = adapter(&home.0, "copilot").list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| session.size_bytes == 0),
        "listing sized a directory outside session-state/: {sessions:?}"
    );
}

#[cfg(unix)]
#[test]
fn antigravity_list_does_not_size_a_directory_outside_brain() {
    let home = TestHome::new();
    fs::create_dir_all(home.0.join(".gemini/antigravity/brain")).unwrap();
    let outside = home.0.join("outside-ag-size");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.bin"), vec![7u8; 4096]).unwrap();
    home.database(
        ".gemini/antigravity/conversation_summaries.db",
        "CREATE TABLE conversation_summaries (
            conversation_id TEXT, title TEXT, preview TEXT, step_count INTEGER,
            last_modified_time TEXT, workspace_uris TEXT
         );
         INSERT INTO conversation_summaries VALUES (
            '../../../outside-ag-size', 'x', '', 0, NULL, NULL
         );",
    );

    let sessions = adapter(&home.0, "antigravity").list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| session.size_bytes == 0),
        "listing sized a directory outside brain/: {sessions:?}"
    );
}

#[cfg(unix)]
#[test]
fn opencode_list_does_not_size_a_directory_outside_the_store() {
    let home = TestHome::new();
    fs::create_dir_all(home.0.join(".local/share/opencode/session_diff")).unwrap();
    let outside = home.0.join("outside-oc");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.bin"), vec![7u8; 4096]).unwrap();
    home.database(
        ".local/share/opencode/opencode.db",
        "CREATE TABLE session (id TEXT);
         INSERT INTO session VALUES ('../../../../outside-oc');",
    );

    let sessions = adapter(&home.0, "opencode").list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| session.size_bytes == 0),
        "listing sized a directory outside the store: {sessions:?}"
    );
}

#[cfg(unix)]
#[test]
fn copilot_does_not_treat_an_outside_file_as_an_active_lock() {
    let home = TestHome::new();
    fs::create_dir_all(home.0.join(".copilot/session-state/.session-operation-locks")).unwrap();
    fs::write(home.0.join("secret.lock"), "ssh-private").unwrap();

    assert!(
        !adapter(&home.0, "copilot").is_session_active("../../../secret"),
        "an outside file was treated as a live Copilot lock"
    );
}
