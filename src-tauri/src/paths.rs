//! Storage layout, re-exported from `orquestra-core`.
//!
//! The implementation moved to `crates/orquestra-core/src/paths` (task #326).
//! These re-exports keep every `crate::paths::…` import, and the crate's
//! public `paths` API, exactly as they were.

pub use orquestra_core::paths::OrquestraPaths;

pub(crate) use orquestra_core::paths::{
    copy_dir_all, create_dir, ensure_scratch_root, fresh_scratch_dir, open_paths, require_home,
    scratch_root, unique_tmp_suffix,
};
// Only the macOS bundle and the Linux icon writers resolve `$HOME` directly;
// Windows asks the known-folder API instead, so the re-export would be dead.
#[cfg(any(not(windows), test))]
pub(crate) use orquestra_core::paths::home_dir;
