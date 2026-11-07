//! Parent-side context for `src-tauri/src/desktop_shortcut.rs`.
//!
//! The module builds a PowerShell script and reads `%APPDATA%`; it needs
//! nothing from the shell except `std`, so the `#[path]` shim is the whole
//! context. What matters here is that the `CommandExt::creation_flags` call and
//! the shortcut paths compile for the Windows target.

#[path = "../../../../src-tauri/src/desktop_shortcut.rs"]
mod shortcut;

/// What the shell calls once per launch from `desktop.rs`.
pub fn create() {
    shortcut::ensure_start_menu_shortcut();
}
