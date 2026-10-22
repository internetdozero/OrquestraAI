//! Single instance guard: ensures only one OrquestraAI GUI process runs at a
//! time, focusing the existing window when the user launches the app again.
//!
//! Three layers, in order of authority:
//!
//! 1. **Socket handshake.** A second launch connects to the socket and asks
//!    the primary to show itself. A socket that accepts the connection is a
//!    live peer by definition — nothing is ever removed in that case.
//! 2. **`flock` on a lockfile.** The primary holds an exclusive,
//!    non-blocking lock for its whole lifetime. `flock` is kernel-tracked:
//!    `kill -9`, SIGTERM and crashes release it automatically, so a dead
//!    primary can never look live. A failed lock attempt means a live peer
//!    holds it, even when its listener is not up yet (startup race).
//! 3. **PID lockfile.** The primary records its PID next to the socket for
//!    diagnostics. PIDs are recycled by the kernel, so the PID is only ever
//!    a hint for logs — the lock in (2) makes the decision.
//!
//! Signal cleanup (`SIGINT`/`SIGTERM`) is best-effort on top: a dedicated
//! `sigwait` thread unlinks the socket when it wins the delivery race. When
//! it loses (or on `kill -9`, which cannot be caught at all), layers 1–2
//! reclaim the orphan on the next launch.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use tauri::{AppHandle, Manager, Runtime};

use crate::paths::OrquestraPaths;

const SOCKET_NAME: &str = "orquestra.sock";
const LOCK_NAME: &str = "orquestra.lock";

/// The primary's exclusive lock, held for the whole process lifetime.
/// The `File` must stay open: closing it releases the `flock`. Keyed by
/// lock path so a changed `HOME` (tests, profiles) never mistakes one
/// home's primary for another's.
static PRIMARY_LOCK: Mutex<Option<(PathBuf, fs::File)>> = Mutex::new(None);

fn locked_guard() -> std::sync::MutexGuard<'static, Option<(PathBuf, fs::File)>> {
    PRIMARY_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Drop a lock held for a different home. In production the home never
/// changes; in tests every case gets a scratch `HOME`.
fn release_foreign_lock(lock: &Path) {
    let mut guard = locked_guard();
    if guard.as_ref().is_some_and(|(path, _)| path != lock) {
        *guard = None;
    }
}

fn socket_path() -> Result<PathBuf, String> {
    let paths = OrquestraPaths::resolve()?;
    Ok(paths.root.join(SOCKET_NAME))
}

fn lock_path() -> Result<PathBuf, String> {
    let paths = OrquestraPaths::resolve()?;
    Ok(paths.root.join(LOCK_NAME))
}

/// PID recorded by the lock holder, if any. Diagnostic only: PIDs recycle,
/// so this never decides liveness — the `flock` does.
fn lock_holder_pid() -> Option<i32> {
    let path = lock_path().ok()?;
    let text = fs::read_to_string(&path).ok()?;
    text.trim().parse::<i32>().ok().filter(|pid| *pid > 0)
}

/// Try to take the primary lock. `Ok(Some(file))` means no live primary
/// exists and the caller now holds the lock; `Ok(None)` means a live peer
/// holds it.
fn try_claim_lock(lock: &std::path::Path) -> Result<Option<fs::File>, String> {
    if let Some(parent) = lock.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock)
        .map_err(|error| format!("failed to open {}: {error}", lock.display()))?;
    // SAFETY: `flock` on our own open fd with no pointer arguments.
    let held = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    if held {
        Ok(Some(file))
    } else {
        Ok(None)
    }
}

/// Record our PID in the lockfile. Best-effort: the lock itself is the
/// authority, the PID is only a hint for operators reading the dir.
fn record_pid(lock: &std::path::Path) {
    let _ = fs::write(lock, std::process::id().to_string());
}

/// Unlink a dead socket path. Real directories are never removed here —
/// only socket files, dead socket paths and stale regular files.
fn prune_dead_path(path: &Path) {
    if fs::symlink_metadata(path)
        .map(|metadata| !metadata.file_type().is_dir())
        .unwrap_or(false)
    {
        let _ = fs::remove_file(path);
    }
}

/// Try to notify an existing instance to show its window. Returns `Ok(true)`
/// if an existing instance was found (and notified, or is at least alive),
/// or `Ok(false)` if we are the primary instance.
///
/// A socket that accepts the connection is live and never touched. A socket
/// that refuses it is only removed after the lock proves no live primary
/// exists — a slow or half-started peer is reported, never unlinked.
pub fn try_activate_existing() -> Result<bool, String> {
    let path = socket_path()?;
    let lock = lock_path()?;
    release_foreign_lock(&lock);

    // Already primary for this home (lock held by us): a live listener
    // means our own window, otherwise prune any dead path and stay primary.
    if locked_guard()
        .as_ref()
        .is_some_and(|(path, _)| path == &lock)
    {
        if let Ok(mut stream) = UnixStream::connect(&path) {
            let _ = stream.write_all(b"show\n");
            return Ok(true);
        }
        prune_dead_path(&path);
        return Ok(false);
    }

    // Live peer: notify and step aside. Nothing is removed, ever.
    if let Ok(mut stream) = UnixStream::connect(&path) {
        let _ = stream.write_all(b"show\n");
        return Ok(true);
    }

    // No listener. The lock decides: a held lock means a live peer whose
    // listener is not up yet (startup race) — report it, do not touch it.
    match try_claim_lock(&lock)? {
        Some(file) => {
            *locked_guard() = Some((lock.clone(), file));
            record_pid(&lock);
            // Our lock proves no live peer uses this path: a leftover from
            // a crash, SIGTERM or kill -9 is safe to unlink.
            prune_dead_path(&path);
            Ok(false)
        }
        None => {
            if let Some(pid) = lock_holder_pid() {
                crate::log_warn!("single-instance peer is starting (lock held by pid {pid})");
            }
            Ok(true)
        }
    }
}

/// Start listening on the Unix socket so subsequent launches can focus this window.
pub fn start_listener<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let path = socket_path()?;
    let lock = lock_path()?;

    // Normally claimed moments ago by `try_activate_existing`; reclaim here
    // if this entry point is ever used on its own. A held-by-other lock
    // means a live primary: refuse instead of stealing its socket.
    release_foreign_lock(&lock);
    if !locked_guard()
        .as_ref()
        .is_some_and(|(path, _)| path == &lock)
    {
        match try_claim_lock(&lock)? {
            Some(file) => {
                *locked_guard() = Some((lock.clone(), file));
                record_pid(&lock);
            }
            None => {
                return Err(
                    "another OrquestraAI instance holds the single-instance lock".to_string(),
                );
            }
        }
    }

    // Exclusive lock held: no live peer can be using this path.
    let _ = fs::remove_file(&path);

    let listener = UnixListener::bind(&path)
        .map_err(|error| format!("failed to bind single-instance socket: {error}"))?;

    install_signal_cleanup();

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

/// Best-effort proactive cleanup on SIGINT/SIGTERM: a dedicated `sigwait`
/// thread unlinks the socket when it wins the delivery race. Correctness
/// never depends on it — a lost race or `kill -9` leaves an orphan that the
/// lock reclaims on the next launch. Idempotent.
fn install_signal_cleanup() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }

    thread::spawn(|| {
        // SAFETY: zeroed `sigset_t` filled only via `sigemptyset`/`sigaddset`;
        // the mask applies to this thread alone so `sigwait` is well-defined.
        let mut watched: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut watched);
            libc::sigaddset(&mut watched, libc::SIGINT);
            libc::sigaddset(&mut watched, libc::SIGTERM);
            libc::pthread_sigmask(libc::SIG_BLOCK, &watched, std::ptr::null_mut());
        }
        loop {
            let mut signo = 0;
            // SAFETY: `watched` names blocked signals; `signo` is a valid out-param.
            if unsafe { libc::sigwait(&watched, &mut signo) } == 0 {
                cleanup();
                // Restore the default disposition and re-raise so the exit
                // status still reflects the signal that killed us.
                unsafe {
                    libc::signal(signo, libc::SIG_DFL);
                    libc::raise(signo);
                }
            }
        }
    });
}

/// Clean up the socket and lock files on exit. Dropping the held fd
/// releases the `flock` (process death does the same automatically), then
/// the names are unlinked best-effort.
pub fn cleanup() {
    *locked_guard() = None;
    if let Ok(path) = socket_path() {
        let _ = fs::remove_file(path);
    }
    if let Ok(lock) = lock_path() {
        let _ = fs::remove_file(lock);
    }
}
