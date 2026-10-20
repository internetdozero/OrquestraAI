use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let root = PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-freebuff-link-{}-{unique}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[cfg(unix)]
#[test]
fn manicode_project_symlink_is_not_listed_or_deleted() {
    let root = scratch();
    let home = root.join("home");
    let outside = root.join("outside");
    fs::create_dir_all(outside.join("chats/demo")).unwrap();
    let secret = outside.join("chats/demo/chat-messages.json");
    let original = "[{\"role\":\"user\",\"content\":\"ssh-private\"}]";
    fs::write(&secret, original).unwrap();
    let projects = home.join(".config/manicode/projects");
    fs::create_dir_all(&projects).unwrap();
    std::os::unix::fs::symlink(&outside, projects.join("linked")).unwrap();

    let adapter = Freebuff::new(&home);
    let sessions = adapter.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| {
            session.id != "linked/demo" && !session.title.contains("ssh-private")
        }),
        "listing followed the project symlink: {sessions:?}"
    );
    assert!(adapter.read_transcript("linked/demo").is_empty());
    adapter.delete_session("linked/demo").unwrap();
    assert_eq!(fs::read_to_string(&secret).unwrap(), original);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn worktree_metadata_does_not_follow_a_directory_symlink() {
    let root = scratch();
    let home = root.join("home");
    let outside = root.join("outside-meta");
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        outside.join("proj.thread.json"),
        "{\"title\":\"ssh-private\",\"cwd\":\"/secret\"}",
    )
    .unwrap();
    let worktree = home.join(".freebuff/worktrees/realwt");
    fs::create_dir_all(&worktree).unwrap();
    std::os::unix::fs::symlink(&outside, worktree.join("nested")).unwrap();

    let sessions = Freebuff::new(&home).list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| {
            !session.title.contains("ssh-private")
                && session.project_path.as_deref() != Some("/secret")
        }),
        "metadata walk followed the symlink: {sessions:?}"
    );
    assert_eq!(
        fs::read_to_string(outside.join("proj.thread.json")).unwrap(),
        "{\"title\":\"ssh-private\",\"cwd\":\"/secret\"}"
    );
    let _ = fs::remove_dir_all(&root);
}
