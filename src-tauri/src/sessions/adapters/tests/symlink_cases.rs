//! Directory-symlink cases that do not fit in the parent test module.

use std::fs;

use super::{adapter, TestHome};

#[cfg(unix)]
#[test]
fn claude_does_not_follow_a_sessions_directory_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let secret = outside.join("abc.jsonl");
    fs::write(
        &secret,
        "{\"type\":\"user\",\"cwd\":\"/secret\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ssh-private\"}]}}\n",
    )
    .unwrap();
    let claude_dir = home.0.join(".claude");
    fs::create_dir_all(claude_dir.join("projects")).unwrap();
    std::os::unix::fs::symlink(&outside, claude_dir.join("sessions")).unwrap();

    let claude = adapter(&home.0, "claude");
    let sessions = claude.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| {
            !session.title.contains("ssh-private")
                && session.project_path.as_deref() != Some("/secret")
        }),
        "listing followed the sessions symlink: {sessions:?}"
    );
    assert!(
        claude
            .read_transcript("abc")
            .iter()
            .all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript followed the sessions symlink"
    );
    claude.delete_session("abc").unwrap();
    assert!(
        secret.is_file(),
        "delete must not remove the transcript outside the store"
    );
    assert!(fs::read_to_string(&secret).unwrap().contains("ssh-private"));
}

#[cfg(unix)]
#[test]
fn claude_does_not_follow_a_projects_directory_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside");
    let project = outside.join("slug");
    fs::create_dir_all(&project).unwrap();
    let secret = project.join("abc.jsonl");
    fs::write(
        &secret,
        "{\"type\":\"user\",\"cwd\":\"/secret\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ssh-private\"}]}}\n",
    )
    .unwrap();
    let claude_dir = home.0.join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    std::os::unix::fs::symlink(&outside, claude_dir.join("projects")).unwrap();

    let claude = adapter(&home.0, "claude");
    let sessions = claude.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| {
            !session.title.contains("ssh-private")
                && session.project_path.as_deref() != Some("/secret")
        }),
        "listing followed the projects symlink: {sessions:?}"
    );
    assert!(
        claude
            .read_transcript("abc")
            .iter()
            .all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript followed the projects symlink"
    );
    claude.delete_session("abc").unwrap();
    assert!(
        secret.is_file(),
        "delete must not remove the transcript outside the store"
    );
    assert!(fs::read_to_string(&secret).unwrap().contains("ssh-private"));
}

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
fn freebuff_does_not_follow_a_desktop_projects_symlink() {
    let home = TestHome::new();
    let thread = "11111111-2222-3333-4444-555555555555";
    home.database(
        "outside/slug/desktop-v2.db",
        &format!(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT);
             CREATE TABLE messages (
                seq INTEGER PRIMARY KEY AUTOINCREMENT, thread_id TEXT,
                role TEXT, parts_json TEXT
             );
             INSERT INTO threads VALUES ('{thread}', 'kept');
             INSERT INTO messages (thread_id, role, parts_json)
             VALUES ('{thread}', 'user', '[{{\"type\":\"text\",\"text\":\"ssh-private\"}}]');"
        ),
    );
    fs::write(
        home.0.join("outside/slug/project.json"),
        "{\"projectPath\":\"/secret\"}",
    )
    .unwrap();
    let desktop = home.0.join(".config/freebuff-desktop");
    fs::create_dir_all(&desktop).unwrap();
    std::os::unix::fs::symlink(home.0.join("outside"), desktop.join("projects")).unwrap();

    let freebuff = adapter(&home.0, "freebuff");
    let sessions = freebuff.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| {
            session.project_path.as_deref() != Some("/secret") && !session.id.contains("slug")
        }),
        "listing followed the desktop projects symlink: {sessions:?}"
    );
    let id = format!("desktop:slug:{thread}");
    assert!(
        freebuff
            .read_transcript(&id)
            .iter()
            .all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript followed the desktop projects symlink"
    );
    freebuff.delete_session(&id).unwrap();
    let left: String = rusqlite::Connection::open(home.0.join("outside/slug/desktop-v2.db"))
        .unwrap()
        .query_row("SELECT title FROM threads", [], |row| row.get(0))
        .unwrap();
    assert_eq!(left, "kept");
}

#[cfg(unix)]
#[test]
fn freebuff_does_not_delete_through_a_local_projects_symlink() {
    let home = TestHome::new();
    let thread = "11111111-2222-3333-4444-555555555555";
    let outside = home.0.join("outside");
    fs::create_dir_all(outside.join(format!("pid/threads/{thread}"))).unwrap();
    let secret = outside.join(format!("pid/threads/{thread}/note.txt"));
    fs::write(&secret, "ssh-private").unwrap();
    let desktop = home.0.join(".config/freebuff-desktop");
    fs::create_dir_all(&desktop).unwrap();
    std::os::unix::fs::symlink(&outside, desktop.join("local-projects")).unwrap();

    adapter(&home.0, "freebuff")
        .delete_session(&format!("desktop:slug:{thread}"))
        .unwrap();
    assert!(
        secret.is_file(),
        "delete must not remove the thread folder outside the store"
    );
    assert_eq!(fs::read_to_string(&secret).unwrap(), "ssh-private");
}
