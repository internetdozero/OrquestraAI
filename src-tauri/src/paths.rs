//! Storage layout, re-exported from `orquestra-core`.
//!
//! The implementation moved to `crates/orquestra-core/src/paths` (task #326).
//! These re-exports keep every `crate::paths::…` import, and the crate's
//! public `paths` API, exactly as they were.

pub use orquestra_core::paths::OrquestraPaths;

pub(crate) use orquestra_core::paths::{
    copy_dir_all, create_dir, fresh_scratch_dir, open_paths, scratch_root, unique_tmp_suffix,
    AGENTS_FILE,
};

// Reached only from the updater's test module, so `cargo check --lib` (which
// drops `cfg(test)` code) must not see it as an unused import.
#[cfg(test)]
pub(crate) use orquestra_core::paths::ensure_scratch_root;
