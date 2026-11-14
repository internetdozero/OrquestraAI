//! Grok session-folder symlinks. The store root is already refused.

use std::fs;

use super::{adapter, TestHome};

#[cfg(unix)]
#[test]
fn grok_does_not_read_through_a_session_directory_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-grok-session");
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        outside.join("chat_history.jsonl"),
        "{\"type\":\"user\",\"content\":\"ssh-private\",\"timestamp\":1791708904}\n",
    )
    .unwrap();
    let project = home.0.join(".grok/sessions/proj");
    fs::create_dir_all(&project).unwrap();
    std::os::unix::fs::symlink(&outside, project.join("sess")).unwrap();

    let turns = adapter(&home.0, "grok").read_transcript("sess");
    assert!(
        turns.is_empty(),
        "a symlinked session folder must not yield {turns:?}"
    );
    assert_eq!(
        fs::read_to_string(outside.join("chat_history.jsonl")).unwrap(),
        "{\"type\":\"user\",\"content\":\"ssh-private\",\"timestamp\":1791708904}\n"
    );
}
