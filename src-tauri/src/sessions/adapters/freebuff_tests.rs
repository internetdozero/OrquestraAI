use super::*;
use std::fs;
use std::io::Write;
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

#[test]
fn manicode_list_does_not_parse_an_oversized_chat_log() {
    let root = scratch();
    let home = root.join("home");
    let log = home.join(".config/manicode/projects/proj/chats/demo/chat-messages.json");
    fs::create_dir_all(log.parent().unwrap()).unwrap();
    let mut file = fs::File::create(&log).unwrap();
    file.write_all(br#"[{"role":"user","content":""#).unwrap();
    let chunk = vec![b'a'; 1024 * 1024];
    for _ in 0..33 {
        file.write_all(&chunk).unwrap();
    }
    file.write_all(br#""}]"#).unwrap();
    drop(file);

    let sessions = Freebuff::new(&home).list_sessions().unwrap();
    let session = sessions
        .iter()
        .find(|session| session.id == "proj/demo")
        .unwrap();
    assert_eq!(
        session.turns_count, 0,
        "listing parsed a chat log past the 32 MiB ceiling"
    );
    assert_eq!(session.title, "Chat demo");
    assert!(log.is_file());
    let _ = fs::remove_dir_all(&root);
}
