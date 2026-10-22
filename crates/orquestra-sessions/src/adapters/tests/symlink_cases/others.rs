//! Copilot, Antigravity, Hermes, OpenCode and Grok stores replaced by links.

use std::fs;

use super::{adapter, TestHome};

#[cfg(unix)]
#[test]
fn copilot_does_not_follow_a_session_state_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside");
    fs::create_dir_all(outside.join("orphan")).unwrap();
    let secret = outside.join("orphan/state.json");
    fs::write(&secret, "ssh-private").unwrap();
    let copilot_dir = home.0.join(".copilot");
    fs::create_dir_all(&copilot_dir).unwrap();
    std::os::unix::fs::symlink(&outside, copilot_dir.join("session-state")).unwrap();

    let copilot = adapter(&home.0, "copilot");
    let sessions = copilot.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| session.id != "orphan"),
        "listing followed the session-state symlink: {sessions:?}"
    );
    copilot.delete_session("orphan").unwrap();
    assert!(
        secret.is_file(),
        "delete must not remove the state folder outside the store"
    );
    assert_eq!(fs::read_to_string(&secret).unwrap(), "ssh-private");
}

#[cfg(unix)]
#[test]
fn antigravity_does_not_follow_a_brain_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside");
    fs::create_dir_all(outside.join("conv/logs")).unwrap();
    let secret = outside.join("conv/logs/transcript.jsonl");
    fs::write(
        &secret,
        "{\"source\":\"USER_EXPLICIT\",\"type\":\"USER_INPUT\",\
          \"content\":\"<USER_REQUEST>ssh-private</USER_REQUEST>\"}\n",
    )
    .unwrap();
    let store = home.0.join(".gemini/antigravity");
    fs::create_dir_all(&store).unwrap();
    std::os::unix::fs::symlink(&outside, store.join("brain")).unwrap();

    let antigravity = adapter(&home.0, "antigravity");
    let sessions = antigravity.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| session.id != "conv"),
        "listing followed the brain symlink: {sessions:?}"
    );
    assert!(
        antigravity
            .read_transcript("conv")
            .iter()
            .all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript followed the brain symlink"
    );
    antigravity.delete_session("conv").unwrap();
    assert!(
        secret.is_file(),
        "delete must not remove the brain folder outside the store"
    );
    assert!(fs::read_to_string(&secret).unwrap().contains("ssh-private"));
}

#[cfg(unix)]
#[test]
fn hermes_does_not_delete_through_a_sessions_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let secret = outside.join("abc.jsonl");
    fs::write(&secret, "ssh-private\n").unwrap();
    let hermes_dir = home.0.join(".hermes");
    fs::create_dir_all(&hermes_dir).unwrap();
    std::os::unix::fs::symlink(&outside, hermes_dir.join("sessions")).unwrap();
    home.database(
        ".hermes/state.db",
        "CREATE TABLE sessions (
            id TEXT, source TEXT, display_name TEXT,
            started_at TEXT, ended_at TEXT, message_count INTEGER
         );
         INSERT INTO sessions VALUES ('abc', '', '', NULL, NULL, 0);",
    );

    let hermes = adapter(&home.0, "hermes");
    let sessions = hermes.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| session.size_bytes == 0),
        "listing measured the log through the sessions symlink: {sessions:?}"
    );
    hermes.delete_session("abc").unwrap();
    assert!(
        secret.is_file(),
        "delete must not remove the log outside the store"
    );
    assert_eq!(fs::read_to_string(&secret).unwrap(), "ssh-private\n");
}

#[cfg(unix)]
#[test]
fn opencode_does_not_delete_through_an_artifact_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside");
    fs::create_dir_all(outside.join("abc")).unwrap();
    let secret = outside.join("abc/note.txt");
    fs::write(&secret, "ssh-private").unwrap();
    let store = home.0.join(".local/share/opencode");
    fs::create_dir_all(&store).unwrap();
    std::os::unix::fs::symlink(&outside, store.join("session_diff")).unwrap();

    adapter(&home.0, "opencode").delete_session("abc").unwrap();
    assert!(
        secret.is_file(),
        "delete must not remove the artifact folder outside the store"
    );
    assert_eq!(fs::read_to_string(&secret).unwrap(), "ssh-private");
}

#[cfg(unix)]
#[test]
fn grok_does_not_follow_a_sessions_directory_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-grok");
    let session = outside.join("proj/sess");
    fs::create_dir_all(&session).unwrap();
    let log = session.join("chat_history.jsonl");
    let original = "{\"type\":\"user\",\"content\":\"ssh-private\",\"timestamp\":1791708904}\n";
    fs::write(&log, original).unwrap();
    fs::create_dir_all(home.0.join(".grok")).unwrap();
    std::os::unix::fs::symlink(&outside, home.0.join(".grok/sessions")).unwrap();

    let grok = adapter(&home.0, "grok");
    let turns = grok.read_transcript("sess");
    assert!(
        turns
            .iter()
            .all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript followed the sessions symlink: {turns:?}"
    );
    grok.delete_session("sess").unwrap();
    assert_eq!(fs::read_to_string(&log).unwrap(), original);
    assert!(session.is_dir(), "the outside session folder must stay");
}

#[cfg(unix)]
#[test]
fn antigravity_does_not_delete_through_a_conversations_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-ag");
    fs::create_dir_all(&outside).unwrap();
    let secret = outside.join("abc.db");
    fs::write(&secret, "ssh-private").unwrap();
    let store = home.0.join(".gemini/antigravity");
    fs::create_dir_all(&store).unwrap();
    std::os::unix::fs::symlink(&outside, store.join("conversations")).unwrap();

    adapter(&home.0, "antigravity")
        .delete_session("abc")
        .unwrap();
    assert_eq!(fs::read_to_string(&secret).unwrap(), "ssh-private");
    assert!(secret.is_file(), "the outside database must stay");
}
