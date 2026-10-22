//! A project folder under a real Freebuff Desktop `projects/` directory.

use std::fs;

use super::{adapter, TestHome};

#[cfg(unix)]
#[test]
fn freebuff_does_not_follow_a_project_directory_symlink() {
    let home = TestHome::new();
    let thread = "11111111-2222-3333-4444-555555555555";
    home.database(
        "outside-desktop/desktop-v2.db",
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
    let projects = home.0.join(".config/freebuff-desktop/projects");
    fs::create_dir_all(&projects).unwrap();
    std::os::unix::fs::symlink(home.0.join("outside-desktop"), projects.join("slug")).unwrap();

    let freebuff = adapter(&home.0, "freebuff");
    let id = format!("desktop:slug:{thread}");
    let turns = freebuff.read_transcript(&id);
    assert!(
        turns
            .iter()
            .all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript followed the project symlink: {turns:?}"
    );
    freebuff.delete_session(&id).unwrap();
    let left: String = rusqlite::Connection::open(home.0.join("outside-desktop/desktop-v2.db"))
        .unwrap()
        .query_row("SELECT title FROM threads", [], |row| row.get(0))
        .unwrap();
    assert_eq!(left, "kept");
}
