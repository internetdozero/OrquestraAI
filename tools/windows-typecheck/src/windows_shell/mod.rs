//! Parent-side context for `src-tauri/src/windows_shell.rs`.
//!
//! The module needs nothing from the shell except its Win32 imports, so the
//! `#[path]` shim is the whole context — but those imports are the point:
//! `ShellExecuteW` and `SetCurrentProcessExplicitAppUserModelID` only resolve
//! with the `Win32_UI_Shell` feature enabled.

#[path = "../../../../src-tauri/src/windows_shell.rs"]
mod shell;

/// What startup calls once, plus the URL path `open_url` uses on Windows.
pub fn identity() {
    shell::declare_app_identity();
    if false {
        let _ = shell::open_url("https://example.com");
    }
}
