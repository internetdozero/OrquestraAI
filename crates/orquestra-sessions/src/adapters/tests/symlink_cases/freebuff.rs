//! Freebuff stores: desktop projects, local projects, worktrees and the
//! Manicode chat tree, each replaced by a link.

use std::fs;

use super::{adapter, TestHome};

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

#[cfg(unix)]
#[test]
fn freebuff_does_not_follow_a_worktrees_directory_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-worktrees");
    fs::create_dir_all(outside.join("wt")).unwrap();
    fs::write(
        outside.join("wt/proj.thread.json"),
        "{\"title\":\"ssh-private\",\"cwd\":\"/secret\"}",
    )
    .unwrap();
    let secret = outside.join("wt/secret.txt");
    fs::write(&secret, "ssh-private").unwrap();
    fs::create_dir_all(home.0.join(".freebuff")).unwrap();
    std::os::unix::fs::symlink(&outside, home.0.join(".freebuff/worktrees")).unwrap();

    let freebuff = adapter(&home.0, "freebuff");
    let sessions = freebuff.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| {
            !session.title.contains("ssh-private")
                && session.project_path.as_deref() != Some("/secret")
        }),
        "listing followed the worktrees symlink: {sessions:?}"
    );
    freebuff.delete_session("wt").unwrap();
    assert_eq!(fs::read_to_string(&secret).unwrap(), "ssh-private");
    assert!(
        outside.join("wt").is_dir(),
        "the outside worktree must stay"
    );
}

#[cfg(unix)]
#[test]
fn freebuff_does_not_follow_a_manicode_projects_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-manicode");
    fs::create_dir_all(outside.join("proj/chats/demo")).unwrap();
    let secret = outside.join("proj/chats/demo/chat-messages.json");
    let original = "[{\"role\":\"user\",\"content\":\"ssh-private\"}]";
    fs::write(&secret, original).unwrap();
    fs::create_dir_all(home.0.join(".config/manicode")).unwrap();
    std::os::unix::fs::symlink(&outside, home.0.join(".config/manicode/projects")).unwrap();

    let freebuff = adapter(&home.0, "freebuff");
    let sessions = freebuff.list_sessions().unwrap();
    assert!(
        sessions
            .iter()
            .all(|session| { session.id != "proj/demo" && !session.title.contains("ssh-private") }),
        "listing followed the projects symlink: {sessions:?}"
    );
    assert!(freebuff.read_transcript("proj/demo").is_empty());
    freebuff.delete_session("proj/demo").unwrap();
    assert_eq!(fs::read_to_string(&secret).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn desktop_delete_does_not_follow_a_worktrees_directory_symlink() {
    let home = TestHome::new();
    let thread = "11111111-2222-3333-4444-555555555555";
    let outside = home.0.join("outside-desktop-wt");
    let folder = outside.join(thread);
    fs::create_dir_all(&folder).unwrap();
    let secret = folder.join("secret.txt");
    fs::write(&secret, "ssh-private").unwrap();
    fs::create_dir_all(home.0.join(".freebuff")).unwrap();
    std::os::unix::fs::symlink(&outside, home.0.join(".freebuff/worktrees")).unwrap();

    adapter(&home.0, "freebuff")
        .delete_session(&format!("desktop:slug:{thread}"))
        .unwrap();
    assert_eq!(fs::read_to_string(&secret).unwrap(), "ssh-private");
    assert!(folder.is_dir(), "the outside worktree must stay");
}
