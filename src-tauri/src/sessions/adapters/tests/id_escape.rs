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
