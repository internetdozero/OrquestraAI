//! Store-level guards: a database or project directory that is a symlink is
//! never opened, sized or deleted through.

use std::fs;

use rusqlite::Connection;

use super::{adapter, TestHome};

#[cfg(unix)]
#[test]
fn sqlite_open_does_not_follow_a_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside.sqlite");
    let real = Connection::open(&outside).unwrap();
    real.execute_batch("CREATE TABLE t(id INTEGER); INSERT INTO t VALUES (1);")
        .unwrap();
    drop(real);
    let link = home.0.join("state.vscdb");
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    assert!(super::super::fsops::open_writable(&link).is_err());
    assert!(super::super::utils::open_readonly(&link).is_err());
    assert!(super::super::fsops::vacuum(&link).is_ok());

    let left: i64 = Connection::open(&outside)
        .unwrap()
        .query_row("SELECT id FROM t", [], |row| row.get(0))
        .unwrap();
    assert_eq!(left, 1);

    let owned = home.0.join("owned.sqlite");
    Connection::open(&owned)
        .unwrap()
        .execute_batch("CREATE TABLE t(id INTEGER);")
        .unwrap();
    assert!(super::super::fsops::open_writable(&owned).is_ok());
}

#[cfg(unix)]
#[test]
fn claude_does_not_follow_a_project_directory_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let secret = outside.join("abc.jsonl");
    fs::write(
        &secret,
        "{\"type\":\"user\",\"cwd\":\"/secret\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ssh-private\"}]}}\n",
    )
    .unwrap();
    let projects = home.0.join(".claude/projects");
    fs::create_dir_all(&projects).unwrap();
    std::os::unix::fs::symlink(&outside, projects.join("linked")).unwrap();

    let claude = adapter(&home.0, "claude");
    let sessions = claude.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| {
            !session.title.contains("ssh-private")
                && session.project_path.as_deref() != Some("/secret")
        }),
        "listing followed the project symlink: {sessions:?}"
    );
    claude.delete_session("abc").unwrap();
    assert!(
        secret.is_file(),
        "delete must not remove the transcript outside the store"
    );
    assert!(fs::read_to_string(&secret).unwrap().contains("ssh-private"));
}
