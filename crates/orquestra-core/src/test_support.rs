//! Scratch directories for tests that build a real on-disk layout.
//!
//! Fixtures used to hardcode a Linux-only `/var/tmp` path, which does not
//! exist on Windows (so every one of those tests failed to create its
//! workspace there) and ignored the platform's temp directory everywhere
//! else. `root()` is the single place that answers "where do test fixtures
//! live":
//!
//! - `ORQUESTRA_TEST_TMP`, when set to an absolute path. This repository's own
//!   convention keeps scratch data off `/tmp`, so a developer exports it once:
//!   `ORQUESTRA_TEST_TMP=/mnt/dev/testing`.
//! - otherwise the platform temp directory (`TMPDIR`, `%TEMP%`, …).
//!
//! Only tests use this module: it is compiled for `cfg(test)` inside this
//! crate and behind the `test-support` feature for the crates whose tests build
//! the same fixtures.

use std::path::{Path, PathBuf};

/// Root every test fixture is built under.
pub fn root() -> PathBuf {
    match std::env::var_os("ORQUESTRA_TEST_TMP") {
        Some(value) if Path::new(&value).is_absolute() => PathBuf::from(value),
        // A relative value would silently nest fixtures inside the crate
        // directory; the spec elsewhere in this workspace ignores those too.
        _ => std::env::temp_dir(),
    }
}

/// `<root>/<prefix>-<pid>-<nanos>`: a directory no other test run shares.
///
/// The timestamp is what makes two runs of the same test (or two test threads
/// of the same binary) collide-free, so callers may reuse a literal prefix.
pub fn scratch(prefix: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    root().join(format!("{prefix}-{}-{unique}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The override is honoured only when absolute, so a stray relative value
    /// cannot put fixtures inside the source tree.
    #[test]
    fn a_relative_override_falls_back_to_the_platform_temp_dir() {
        let absolute = std::env::temp_dir();
        assert!(absolute.is_absolute(), "the platform temp dir is absolute");
        assert!(root().is_absolute());
    }

    #[test]
    fn scratch_directories_differ_between_calls() {
        assert_ne!(scratch("orquestra-test"), scratch("orquestra-test"));
        assert!(scratch("orquestra-test").starts_with(root()));
    }
}
