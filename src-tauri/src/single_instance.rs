//! Single instance guard: ensures only one OrquestraAI GUI process runs at a
//! time, focusing the existing window when the user launches the app again.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use tauri::{AppHandle, Manager, Runtime};

use crate::paths::OrquestraPaths;

const SOCKET_NAME: &str = "orquestra.sock";

fn socket_path() -> Result<PathBuf, String> {
    let paths = OrquestraPaths::resolve()?;
    Ok(paths.root.join(SOCKET_NAME))
}

/// Try to notify an existing instance to show its window. Returns `Ok(true)` if
/// an existing instance was found and notified, or `Ok(false)` if we are the
/// primary instance.
pub fn try_activate_existing() -> Result<bool, String> {
    let path = socket_path()?;
    if !path.exists() {
        return Ok(false);
    }

    // Try connecting to see if the socket is alive.
    if let Ok(mut stream) = UnixStream::connect(&path) {
        let _ = stream.write_all(b"show\n");
        return Ok(true);
    }

    // Stale socket from an earlier crashed process: remove it.
    let _ = fs::remove_file(&path);
    Ok(false)
}

/// Start listening on the Unix socket so subsequent launches can focus this window.
pub fn start_listener<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let path = socket_path()?;
    let _ = fs::remove_file(&path);

    let listener = UnixListener::bind(&path)
        .map_err(|e| format!("failed to bind single-instance socket: {e}"))?;

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = [0u8; 16];
                    let _ = stream.read(&mut buf);
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
                Err(_) => break,
            }
        }
    });

    Ok(())
}

/// Clean up the socket file on exit.
pub fn cleanup() {
    if let Ok(path) = socket_path() {
        let _ = fs::remove_file(path);
    }
}
