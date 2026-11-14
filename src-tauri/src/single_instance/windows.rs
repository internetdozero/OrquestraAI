//! Named-mutex ownership for Windows, held for the process lifetime.
//!
//! `Local\` scopes the name to the logon session: a second operator's session
//! (fast user switching, RDP) keeps its own instance instead of being told an
//! unrelated window is running. The name carries the same store-root hash as
//! the socket path so two profiles on one machine never collide.

use std::sync::Mutex;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;

use crate::paths::OwnHubPaths;

/// The handle held in the `static` below.
///
/// `HANDLE` is a raw pointer, and raw pointers are not `Send`, so
/// `Mutex<Option<HANDLE>>` is not `Sync` and cannot live in a `static` at all.
struct OwnedMutex(HANDLE);

// SAFETY: a kernel handle is an opaque process-wide token, not a pointer into
// thread-local memory. Acquiring and closing it from different threads is the
// documented usage (`CloseHandle` may be called from any thread).
unsafe impl Send for OwnedMutex {}

static OWNED: Mutex<Option<OwnedMutex>> = Mutex::new(None);

fn owned() -> std::sync::MutexGuard<'static, Option<OwnedMutex>> {
    OWNED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// NUL-terminated UTF-16 name, from the store root's stable hash.
pub(super) fn mutex_name() -> Result<Vec<u16>, String> {
    let root = OwnHubPaths::resolve()?.root;
    let key = super::short_key(root.to_string_lossy().as_ref());
    let mut name: Vec<u16> = format!("Local\\ownhub-{key}").encode_utf16().collect();
    name.push(0);
    Ok(name)
}

/// Try to become the primary instance. `Ok(true)` means this process owns the
/// mutex now; `Ok(false)` means another process holds it.
pub(super) fn acquire() -> Result<bool, String> {
    if owned().is_some() {
        return Ok(true);
    }

    let name = mutex_name()?;
    // SAFETY: `name` is NUL-terminated UTF-16 and outlives the call; a NULL
    // security descriptor with a non-inheritable handle is the documented
    // "current user, this process" form.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        // SAFETY: `GetLastError` reads thread-local state and has no
        // preconditions.
        let code = unsafe { GetLastError() };
        return Err(format!(
            "could not create the instance mutex (error {code})"
        ));
    }

    // `ERROR_ALREADY_EXISTS` means the name existed before this call: a live
    // peer owns it. We hold a duplicate handle, so close it and step aside.
    // SAFETY: same thread-local read as above.
    let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if already_exists {
        // SAFETY: `handle` came from `CreateMutexW` and is not used again.
        unsafe { CloseHandle(handle) };
        return Ok(false);
    }

    *owned() = Some(OwnedMutex(handle));
    Ok(true)
}

/// Release the mutex handle (process exit does this too).
pub(super) fn release() {
    let mut guard = owned();
    if let Some(handle) = guard.take() {
        // SAFETY: `handle.0` came from `CreateMutexW` and is closed once.
        unsafe { CloseHandle(handle.0) };
    }
}
