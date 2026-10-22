//! The operator's home directory, resolved in one place.
//!
//! Everything in the app hangs off it: the vault and its projections, every
//! harness session store, the idea vault, the desktop entries. Each call site
//! used to read `HOME` for itself, which meant nine copies of the same lookup
//! and no answer at all on Windows, where the variable is `USERPROFILE`.
//!
//! Resolution takes the lookup as an argument ([`home_dir_with`]) the same way
//! harness install resolution does, so a test can describe an environment
//! without mutating the process-global one that sibling tests read.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::{CoreError, CoreResult};

/// Variables consulted, in order. `HOME` comes first because it is what Unix
/// sets — and what the test harness points at a scratch tree — with
/// `USERPROFILE` behind it for Windows.
pub const HOME_VARS: [&str; 2] = ["HOME", "USERPROFILE"];

/// Named as a whole in error messages, so an operator on either platform sees
/// every variable that would have satisfied the lookup.
const HOME_LABEL: &str = "HOME or USERPROFILE";

/// The home directory named by the environment, if any.
pub fn home_dir() -> Option<PathBuf> {
    // Wrapped in a closure: `var_os` is generic over `AsRef<OsStr>`, so its
    // item type does not satisfy the `Fn(&str)` bound on its own.
    home_dir_with(|name: &str| std::env::var_os(name))
}

/// [`home_dir`] against an injected lookup.
///
/// A variable set to an empty or whitespace-only value is skipped instead of
/// being taken as a location — a shell that exports `HOME=` should not send the
/// vault to `/`. The accepted value itself is never trimmed: a path is used
/// exactly as the operator wrote it.
pub fn home_dir_with(lookup: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    HOME_VARS
        .iter()
        .find_map(|name| lookup(name).filter(names_a_directory))
        .map(PathBuf::from)
}

/// [`home_dir`], or the canonical missing-environment error carrying `purpose`
/// (which completes "cannot …", e.g. `locate ~/.orquestra`).
pub fn require_home(purpose: &'static str) -> CoreResult<PathBuf> {
    require_home_with(|name: &str| std::env::var_os(name), purpose)
}

/// [`require_home`] against an injected lookup.
pub fn require_home_with(
    lookup: impl Fn(&str) -> Option<OsString>,
    purpose: &'static str,
) -> CoreResult<PathBuf> {
    home_dir_with(lookup).ok_or_else(|| CoreError::missing_env(HOME_LABEL, purpose))
}

/// Whether a variable's value could name a directory at all.
fn names_a_directory(value: &OsString) -> bool {
    !value.is_empty() && !value.to_string_lossy().trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lookup over a fixed table: no environment, no lock, no flakes.
    fn lookup(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        let pairs = pairs.to_vec();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(*value))
        }
    }

    #[test]
    fn home_wins_over_userprofile() {
        let home = home_dir_with(lookup(&[
            ("HOME", "/home/system"),
            ("USERPROFILE", "C:\\Users\\system"),
        ]));
        assert_eq!(home, Some(PathBuf::from("/home/system")));
    }

    #[test]
    fn userprofile_is_the_windows_fallback() {
        let home = home_dir_with(lookup(&[("USERPROFILE", "C:\\Users\\system")]));
        assert_eq!(home, Some(PathBuf::from("C:\\Users\\system")));
    }

    #[test]
    fn empty_and_whitespace_values_are_skipped() {
        let empty = lookup(&[("HOME", ""), ("USERPROFILE", "")]);
        assert_eq!(home_dir_with(empty), None);

        let blank = lookup(&[("HOME", "   "), ("USERPROFILE", "\t")]);
        assert_eq!(home_dir_with(blank), None, "whitespace is not a location");

        let recovered = lookup(&[("HOME", ""), ("USERPROFILE", "C:\\Users\\system")]);
        assert_eq!(
            home_dir_with(recovered),
            Some(PathBuf::from("C:\\Users\\system")),
            "an empty HOME does not hide the fallback"
        );
    }

    #[test]
    fn a_missing_home_names_both_variables() {
        let error = require_home_with(|_| None, "locate ~/.orquestra").expect_err("no home");
        assert_eq!(
            error.to_string(),
            "HOME or USERPROFILE is not set; cannot locate ~/.orquestra"
        );
    }
}
