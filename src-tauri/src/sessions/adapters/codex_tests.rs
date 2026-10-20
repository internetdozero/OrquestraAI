use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
#[test]
fn sessions_symlink_is_not_listed_or_deleted() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let root = PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-codex-link-{}-{unique}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let home = root.join("home");
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(home.join(".codex")).unwrap();
    let id = "01a148f4-9277-7820-8e2c-30f21ca5100f";
    let secret = outside.join(format!("rollout-2026-10-17T05-22-30-{id}.jsonl"));
    let original = "{\"payload\":{\"cwd\":\"/secret\"}}\n";
    fs::write(&secret, original).unwrap();
    std::os::unix::fs::symlink(&outside, home.join(".codex/sessions")).unwrap();

    let codex = Codex::new(&home);
    let sessions = codex.list_sessions().unwrap();
    assert!(
        sessions.iter().all(|session| {
            session.id != id && session.project_path.as_deref() != Some("/secret")
        }),
        "listing followed sessions symlink: {sessions:?}"
    );
    codex.delete_session(id).unwrap();
    assert_eq!(fs::read_to_string(&secret).unwrap(), original);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn maintenance_lock_symlink_does_not_block_delete() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let root = PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-codex-lock-{}-{unique}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let home = root.join("home");
    let outside = root.join("outside");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(&outside, "keep\n").unwrap();
    std::os::unix::fs::symlink(&outside, home.join(".codex/.sqlite-maintenance.lock")).unwrap();

    let result = Codex::new(&home).delete_session("01a148f4-9277-7820-8e2c-30f21ca5100f");
    assert!(
        result.is_ok(),
        "a symlink was treated as the maintenance lock: {result:?}"
    );
    assert_eq!(fs::read_to_string(&outside).unwrap(), "keep\n");
    let _ = fs::remove_dir_all(&root);
}
