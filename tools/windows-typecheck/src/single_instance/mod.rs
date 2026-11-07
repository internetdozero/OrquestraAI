//! Parent-side context for `src-tauri/src/single_instance/windows.rs`.
//!
//! The real module hashes the store root into the mutex name through the
//! parent's `short_key`, which is private there and therefore restated here.
//! The two copies must agree; the property that matters (a stable name per
//! store root) is exercised by the Unix single-instance suite, which hashes
//! with the real function.

/// Stable 64-bit FNV-1a of the store root, as 16 lowercase hex digits.
pub(super) fn short_key(input: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The real guard, compiled from the desktop shell.
#[path = "../../../../src-tauri/src/single_instance/windows.rs"]
mod windows;

/// What the shell calls before starting the listener.
pub fn acquire() -> Result<bool, String> {
    windows::acquire()
}

/// What the shell calls on shutdown; also keeps the handle field reachable so
/// `-D warnings` checks it instead of reporting dead code.
pub fn release() {
    windows::release();
}
