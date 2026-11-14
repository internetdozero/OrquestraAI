//! Claude stores: `sessions` and `projects` directories replaced by links.

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
