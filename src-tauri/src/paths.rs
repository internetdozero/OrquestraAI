//! Storage layout, re-exported from `ownhub-core`.
//!
//! The implementation moved to `crates/ownhub-core/src/paths` (task #326).
//! These re-exports keep every `crate::paths::…` import, and the crate's
//! public `paths` API, exactly as they were.

pub use ownhub_core::paths::OwnHubPaths;

pub(crate) use ownhub_core::paths::{
    copy_dir_all, create_dir, ensure_scratch_root, fresh_scratch_dir, open_paths, require_home,
    scratch_root, storage_root, unique_tmp_suffix,
};
// Only the macOS bundle and the Linux icon writers resolve `$HOME` directly;
// Windows asks the known-folder API instead, so the re-export would be dead.
#[cfg(any(not(windows), test))]
pub(crate) use ownhub_core::paths::home_dir;
