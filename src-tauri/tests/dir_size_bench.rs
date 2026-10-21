//! Ignored benchmark for session directory sizing (task #296).
//!
//! Builds a 50-session fixture where every session looks like a real
//! worktree — a couple of real files plus `node_modules/`, `.git/`,
//! `target/` and `__pycache__/` ballast — then times a full `dir_size`
//! pass over all of them. Run with:
//!
//! ```sh
//! cargo test --test dir_size_bench -- --ignored --nocapture
//! ```
//!
//! The test asserts nothing version-specific (only that real files still
//! count), so it runs before and after the fix for a before/after number.

use std::fs;
use std::path::PathBuf;

use orquestra_ai_lib::sessions::adapters::utils::dir_size;

const SESSIONS: usize = 50;
const REAL_BYTES: u64 = 2 * 2048;

fn scratch() -> PathBuf {
    std::env::temp_dir().join(format!("orquestra-dir-bench-{}", std::process::id()))
}

fn write_sized(path: &PathBuf, bytes: usize) {
    fs::write(path, vec![7u8; bytes]).expect("fixture file");
}

#[test]
#[ignore]
fn bench_fifty_session_listing() {
    let root = scratch();
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale fixture");
    }

    for session in 0..SESSIONS {
        let base = root.join(format!("session-{session:02}"));
        fs::create_dir_all(&base).expect("session dir");
        write_sized(&base.join("thread.json"), 2048);
        write_sized(&base.join("meta.json"), 2048);

        let ballast: &[(&str, usize)] = &[
            ("node_modules", 120),
            (".git", 60),
            ("target", 30),
            ("__pycache__", 8),
        ];
        for (dir, files) in ballast {
            let heavy = base.join(dir);
            fs::create_dir_all(&heavy).expect("ballast dir");
            for file in 0..*files {
                write_sized(&heavy.join(format!("blob-{file}.bin")), 1024);
            }
        }
    }

    let started = std::time::Instant::now();
    let mut total = 0u64;
    for session in 0..SESSIONS {
        total += dir_size(&root.join(format!("session-{session:02}")));
    }
    let elapsed = started.elapsed();

    println!("sessions: {SESSIONS}");
    println!("reported_bytes: {total}");
    println!("elapsed_ms: {}", elapsed.as_millis());

    // Real payload files must always count, whatever the walk prunes.
    assert!(
        total >= SESSIONS as u64 * REAL_BYTES,
        "real payload files must always count"
    );

    fs::remove_dir_all(&root).expect("fixture cleanup");
}
