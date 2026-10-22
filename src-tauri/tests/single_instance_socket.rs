#![cfg(unix)]

//! Integration coverage for the single-instance socket lifecycle: primary
//! detection, stale-socket recovery, and handoff to a live listener. The
//! real `start_listener` needs a Tauri `AppHandle`, so the live-listener leg
//! binds a plain `UnixListener` at the same socket path — `connect` is all
//! `try_activate_existing` exercises. Unix-only: the guard is a no-op
//! elsewhere.

mod common;

use std::os::unix::net::UnixListener;

use orquestra_ai_lib::paths::OrquestraPaths;
use orquestra_ai_lib::single_instance::try_activate_existing;

fn store_root() -> std::path::PathBuf {
    OrquestraPaths::resolve().expect("resolve paths").root
}

fn socket_path() -> std::path::PathBuf {
    store_root().join("orquestra.sock")
}

fn lock_path() -> std::path::PathBuf {
    store_root().join("orquestra.lock")
}

#[test]
fn primary_instance_detected_without_socket() {
    common::with_temp_home("single-primary", |_| {
        assert!(!try_activate_existing().expect("check"));
    });
}

#[test]
fn stale_regular_file_is_reclaimed() {
    common::with_temp_home("single-stale", |_| {
        let socket = socket_path();
        std::fs::create_dir_all(socket.parent().expect("parent")).expect("parent dir");
        // A crashed process leaves a regular file behind (or a dead socket):
        // not connectable, so it is removed and we become primary.
        std::fs::write(&socket, b"stale").expect("stale file");
        assert!(!try_activate_existing().expect("check"));
        assert!(!socket.exists(), "stale file must be reclaimed");
    });
}

#[test]
fn crashed_primary_orphan_is_reclaimed() {
    // `kill -9` simulation: a bound listener dropped without cleanup leaves
    // the path behind with no process, plus a lockfile naming a dead PID.
    // The next launch must prove staleness (no handshake + free lock) and
    // reclaim both names.
    common::with_temp_home("single-crash", |_| {
        let socket = socket_path();
        let lock = lock_path();
        std::fs::create_dir_all(store_root()).expect("store dir");
        {
            let _listener = UnixListener::bind(&socket).expect("bind");
            // Far above any real PID: `kill(pid, 0)` reports it dead.
            std::fs::write(&lock, "999999999").expect("stale pid");
        }
        assert!(socket.exists(), "the orphan path must be present");
        assert!(!try_activate_existing().expect("check"));
        assert!(!socket.exists(), "orphan socket must be reclaimed");
        let recorded: i32 = std::fs::read_to_string(&lock)
            .expect("pid file")
            .trim()
            .parse()
            .expect("our pid");
        assert_eq!(recorded, std::process::id() as i32);
    });
}

#[test]
fn own_pid_lockfile_does_not_block_startup() {
    // The lockfile names a live PID (ours) but nobody holds the `flock`:
    // the lock — not the PID hint — decides, so startup proceeds.
    common::with_temp_home("single-ownpid", |_| {
        let lock = lock_path();
        std::fs::create_dir_all(store_root()).expect("store dir");
        std::fs::write(&lock, std::process::id().to_string()).expect("pid file");
        assert!(!try_activate_existing().expect("check"));
    });
}

#[test]
fn live_lock_holder_is_reported_not_unlinked() {
    // A live peer whose listener is not up yet (startup race): the lock is
    // held, the socket is absent. Must report an existing instance and
    // create or remove nothing.
    common::with_temp_home("single-livelock", |_| {
        use std::os::unix::io::AsRawFd;
        let socket = socket_path();
        let lock = lock_path();
        std::fs::create_dir_all(store_root()).expect("store dir");
        std::fs::write(&lock, "424242").expect("pid fixture");
        let holder = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock)
            .expect("open lock");
        let claimed = unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        assert_eq!(claimed, 0, "fixture must hold the lock");
        assert!(try_activate_existing().expect("check"));
        assert!(!socket.exists(), "no socket may be created");
        assert_eq!(
            std::fs::read_to_string(&lock).expect("pid fixture"),
            "424242",
            "a live peer's lockfile must not be touched"
        );
    });
}

#[test]
fn live_listener_claims_secondary() {
    common::with_temp_home("single-live", |_| {
        let socket = socket_path();
        std::fs::create_dir_all(socket.parent().expect("parent")).expect("parent dir");
        let listener = UnixListener::bind(&socket).expect("bind listener");
        assert!(try_activate_existing().expect("check"));
        drop(listener);
        // The listener is gone: the path is a dead socket now, so the next
        // launch reclaims it and becomes primary.
        assert!(!try_activate_existing().expect("check"));
        assert!(!socket.exists(), "dead socket must be reclaimed");
    });
}
