//! Type-checks Windows-only modules of the desktop shell from a non-Windows
//! host.
//!
//! `cargo check --target x86_64-pc-windows-msvc -p orquestra-ai` stops long
//! before this crate's own code: `ring`'s build script and `rusqlite`'s
//! bundled SQLite need a Windows compiler, so the shell — the only crate that
//! contains `#[cfg(windows)]` modules — never gets parsed. That left the
//! Windows collectors one typo away from a red release build.
//!
//! The fix is a staging area, not a copy: each shim module below declares the
//! real source file with `#[path]` and supplies the small amount of context it
//! expects from its parent (`super::`). Nothing here is compiled into a
//! product; the crate exists so `cargo clippy --target …` sees the real code.
//!
//! Adding another module: create `src/<area>/mod.rs` with the parent-side
//! items the module imports, `#[path]` it to the real file, and declare it
//! here. `scripts/windows-typecheck.sh` picks the whole crate up automatically.

pub mod health;

/// Stand-in for the shell's `crate::paths`: the collectors only read storage
/// paths, and [`scratch_root`] is the one the Windows health snapshot sizes.
pub mod paths {
    /// Windows scratch root. Only its capacity is read, so the temp directory
    /// stands in for `%LOCALAPPDATA%\orquestra-ai`.
    pub fn scratch_root() -> std::path::PathBuf {
        std::env::temp_dir()
    }
}

/// Reachable entry point: without a caller the whole graph is dead code and
/// `-D warnings` would fail on every item instead of checking it.
pub fn build_graph() {
    let _ = health::snapshot();
}
