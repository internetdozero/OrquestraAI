//! One conversation folder inside a real Antigravity `brain/` directory.

use std::fs;

use super::{adapter, TestHome};

#[cfg(unix)]
#[test]
fn antigravity_does_not_read_through_a_brain_session_symlink() {
    let home = TestHome::new();
    let outside = home.0.join("outside-brain-session");
    fs::create_dir_all(outside.join("logs")).unwrap();
    let secret = outside.join("logs/transcript.jsonl");
    fs::write(
        &secret,
        "{\"source\":\"USER_EXPLICIT\",\"type\":\"USER_INPUT\",\
          \"content\":\"<USER_REQUEST>ssh-private</USER_REQUEST>\"}\n",
    )
    .unwrap();
    let brain = home.0.join(".gemini/antigravity/brain");
    fs::create_dir_all(&brain).unwrap();
    std::os::unix::fs::symlink(&outside, brain.join("conv")).unwrap();

    let turns = adapter(&home.0, "antigravity").read_transcript("conv");
    assert!(
        turns.iter().all(|turn| !turn.content.contains("ssh-private")),
        "read_transcript followed the brain session symlink: {turns:?}"
    );
    assert!(secret.is_file(), "the outside transcript must stay");
}
