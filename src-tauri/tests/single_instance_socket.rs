//! Integration coverage for the single-instance socket lifecycle: primary
//! detection, stale-socket recovery, and handoff to a live listener. The
//! real `start_listener` needs a Tauri `AppHandle`, so the live-listener leg
//! binds a plain `UnixListener` at the same socket path — `connect` is all
//! `try_activate_existing` exercises.

mod common;

use std::os::unix::net::UnixListener;

use orquestra_ai_lib::paths::OrquestraPaths;
use orquestra_ai_lib::single_instance::try_activate_existing;

fn socket_path() -> std::path::PathBuf {
    OrquestraPaths::resolve()
        .expect("resolve paths")
        .root
        .join("orquestra.sock")
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
