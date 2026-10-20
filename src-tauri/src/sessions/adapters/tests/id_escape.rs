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
        turns
            .iter()
            .all(|turn| !turn.content.contains("ssh-private")),
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
        turns
            .iter()
            .all(|turn| !turn.content.contains("ssh-private")),
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
    fs::create_dir_all(
        home.0
            .join(".copilot/session-state/.session-operation-locks"),
    )
    .unwrap();
    fs::write(home.0.join("secret.lock"), "ssh-private").unwrap();

    assert!(
        !adapter(&home.0, "copilot").is_session_active("../../../secret"),
        "an outside file was treated as a live Copilot lock"
    );
}

#[cfg(unix)]
#[test]
fn copilot_symlink_lock_is_not_an_active_session() {
    let home = TestHome::new();
    let locks = home
        .0
        .join(".copilot/session-state/.session-operation-locks");
    fs::create_dir_all(&locks).unwrap();
    let outside = home.0.join("outside-lock");
    fs::write(&outside, "keep\n").unwrap();
    std::os::unix::fs::symlink(&outside, locks.join("live.lock")).unwrap();
    assert!(
        !adapter(&home.0, "copilot").is_session_active("live"),
        "a symlink lock followed to an outside file"
    );

    let home = TestHome::new();
    let outside_dir = home.0.join("outside-locks");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(outside_dir.join("live.lock"), "keep\n").unwrap();
    fs::create_dir_all(home.0.join(".copilot/session-state")).unwrap();
    std::os::unix::fs::symlink(
        &outside_dir,
        home.0
            .join(".copilot/session-state/.session-operation-locks"),
    )
    .unwrap();
    assert!(
        !adapter(&home.0, "copilot").is_session_active("live"),
        "a symlinked lock directory was followed"
    );
}

#[cfg(unix)]
#[test]
fn antigravity_delete_does_not_follow_a_brain_child_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-ag-delete");
    fs::create_dir_all(outside.join("secret")).unwrap();
    let secret = outside.join("secret/keep.txt");
    fs::write(&secret, "ssh-private\n").unwrap();
    let brain = home.0.join(".gemini/antigravity/brain");
    fs::create_dir_all(&brain).unwrap();
    std::os::unix::fs::symlink(&outside, brain.join("escape")).unwrap();

    adapter(&home.0, "antigravity")
        .delete_session("escape/secret")
        .unwrap();
    assert_eq!(
        fs::read_to_string(&secret).unwrap(),
        "ssh-private\n",
        "delete_session followed brain/escape into the outside tree"
    );
}

#[cfg(unix)]
#[test]
fn hermes_delete_does_not_follow_a_sessions_child_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-hermes-delete");
    fs::create_dir_all(outside.join("secret")).unwrap();
    let secret = outside.join("secret/keep.txt");
    fs::write(&secret, "ssh-private\n").unwrap();
    let logs = home.0.join(".hermes/sessions");
    fs::create_dir_all(&logs).unwrap();
    std::os::unix::fs::symlink(&outside, logs.join("escape")).unwrap();

    adapter(&home.0, "hermes")
        .delete_session("escape/secret")
        .unwrap();
    assert_eq!(
        fs::read_to_string(&secret).unwrap(),
        "ssh-private\n",
        "delete_session followed sessions/escape into the outside tree"
    );
}

#[cfg(unix)]
#[test]
fn copilot_delete_does_not_follow_a_state_child_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-copilot-delete");
    fs::create_dir_all(outside.join("secret")).unwrap();
    let secret = outside.join("secret/keep.txt");
    fs::write(&secret, "ssh-private\n").unwrap();
    let state = home.0.join(".copilot/session-state");
    fs::create_dir_all(&state).unwrap();
    std::os::unix::fs::symlink(&outside, state.join("escape")).unwrap();

    adapter(&home.0, "copilot")
        .delete_session("escape/secret")
        .unwrap();
    assert_eq!(
        fs::read_to_string(&secret).unwrap(),
        "ssh-private\n",
        "delete_session followed session-state/escape into the outside tree"
    );
}

#[cfg(unix)]
#[test]
fn grok_delete_does_not_follow_a_project_child_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-grok-delete");
    fs::create_dir_all(outside.join("secret")).unwrap();
    let secret = outside.join("secret/keep.txt");
    fs::write(&secret, "ssh-private\n").unwrap();
    let project = home.0.join(".grok/sessions/proj");
    fs::create_dir_all(&project).unwrap();
    std::os::unix::fs::symlink(&outside, project.join("escape")).unwrap();

    adapter(&home.0, "grok")
        .delete_session("escape/secret")
        .unwrap();
    assert_eq!(
        fs::read_to_string(&secret).unwrap(),
        "ssh-private\n",
        "delete_session followed sessions/proj/escape into the outside tree"
    );
}

#[cfg(unix)]
#[test]
fn claude_delete_does_not_follow_a_project_child_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-claude-delete");
    fs::create_dir_all(&outside).unwrap();
    let secret = outside.join("secret.jsonl");
    fs::write(&secret, "ssh-private\n").unwrap();
    let project = home.0.join(".claude/projects/proj");
    fs::create_dir_all(&project).unwrap();
    std::os::unix::fs::symlink(&outside, project.join("escape")).unwrap();

    adapter(&home.0, "claude")
        .delete_session("escape/secret")
        .unwrap();
    assert_eq!(
        fs::read_to_string(&secret).unwrap(),
        "ssh-private\n",
        "delete_session followed projects/proj/escape into the outside tree"
    );
}
