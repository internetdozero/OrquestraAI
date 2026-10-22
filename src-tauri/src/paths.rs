//! Storage layout, re-exported from `orquestra-core`.
//!
//! The implementation moved to `crates/orquestra-core/src/paths` (task #326).
//! These re-exports keep every `crate::paths::…` import, and the crate's
//! public `paths` API, exactly as they were.

pub use orquestra_core::paths::OrquestraPaths;

pub(crate) use orquestra_core::paths::{
    copy_dir_all, create_dir, fresh_scratch_dir, open_paths, require_home, scratch_root,
    unique_tmp_suffix,
};

// Reached only from the Linux desktop-integration refresh, so a macOS or
// Windows build (where nothing calls it) must not see it as an unused import.
#[cfg(target_os = "linux")]
pub(crate) use orquestra_core::paths::home_dir;

// Reached only from the updater's test module, so `cargo check --lib` (which
// drops `cfg(test)` code) must not see it as an unused import.
#[cfg(test)]
pub(crate) use orquestra_core::paths::ensure_scratch_root;
