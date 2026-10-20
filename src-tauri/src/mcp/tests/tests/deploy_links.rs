//! Symlink cases for MCP deploy that do not fit in `deploy_cases.rs`.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::super::deploy::deploy_at;

#[cfg(unix)]
#[test]
fn deploy_does_not_follow_a_config_directory_symlink() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-mcp-dir-link-{}-{unique}",
        std::process::id()
    ));
    let home = base.join("home");
    let outside = base.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(home.join(".config")).unwrap();
    let original = "{\n  \"secret\": \"ssh-private\"\n}\n";
    let outside_file = outside.join("opencode.jsonc");
    fs::write(&outside_file, original).unwrap();
    std::os::unix::fs::symlink(&outside, home.join(".config/opencode")).unwrap();

    let root = base.join("orquestra");
    let seed = base.join("seed.json");
    fs::write(
        &seed,
        r#"{
  "headroom": {
    "id": "headroom",
    "name": "Headroom",
    "description": "local",
    "command": "headroom",
    "args": ["mcp"],
    "env": {},
    "enabled": true
  }
}
"#,
    )
    .unwrap();
    let paths = crate::paths::OrquestraPaths {
        vault: root.join("skills-library"),
        active: root.join("active"),
        root,
    };

    let _ = deploy_at(&home, &paths, &seed);
    assert_eq!(
        fs::read_to_string(&outside_file).unwrap(),
        original,
        "deploy must not rewrite a config reached through a directory symlink"
    );
    let _ = fs::remove_dir_all(&base);
}
