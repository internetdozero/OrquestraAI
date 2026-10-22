//! A session file that is a symlink is never sized, parsed or truncated
//! through.

use std::fs;

use crate::adapters::{fsops, transcript, utils};

#[cfg(unix)]
#[test]
fn a_symlinked_session_file_is_not_sized_or_loaded() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let scratch = std::path::PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-file-size-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&scratch).expect("scratch");
    let outside = scratch.join("outside.jsonl");
    let link = scratch.join("session.jsonl");
    let real = scratch.join("real.jsonl");
    let line = "{\"role\":\"user\",\"content\":\"secret-outside\"}\n";
    fs::write(&outside, line).expect("outside jsonl");
    fs::write(&real, "{\"role\":\"user\",\"content\":\"kept\"}\n").expect("real jsonl");
    std::os::unix::fs::symlink(&outside, &link).expect("file symlink");

    assert_eq!(utils::file_size(&link), 0);
    assert_eq!(
        fsops::size_of(&link),
        0,
        "reclaim estimates must not follow a file symlink"
    );
    assert!(utils::file_size(&real) > 0);
    assert!(
        utils::read_jsonl(&link).is_empty(),
        "the size ceiling must not treat a symlink as an empty file and then read it"
    );
    assert_eq!(utils::read_jsonl(&real)[0]["content"], "kept");
    assert!(transcript::read_transcript(&link).is_empty());
    assert_eq!(transcript::read_transcript(&real)[0].content, "kept");
    assert!(outside.is_file(), "the target stays put");

    fs::remove_dir_all(&scratch).expect("cleanup");
}
