//! Session ids that climb out of a real store directory with `..`.

use std::fs;

use super::{adapter, TestHome};

#[cfg(unix)]
#[test]
fn antigravity_read_does_not_escape_brain_with_parent_dirs() {
    let home = TestHome::new();
    let outside = home.0.join("outside-escape");
    fs::create_dir_all(outside.join("logs")).unwrap();
    fs::write(
        outside.join("logs/transcript.jsonl"),
        "{\"source\":\"USER_EXPLICIT\",\"type\":\"USER_INPUT\",\
          \"content\":\"<USER_REQUEST>ssh-private</USER_REQUEST>\"}\n",
    )
    .unwrap();
    fs::create_dir_all(home.0.join(".gemini/antigravity/brain")).unwrap();

    let turns = adapter(&home.0, "antigravity").read_transcript("../../../outside-escape");
    assert!(
        turns.iter().all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript climbed out of brain/: {turns:?}"
    );
}
