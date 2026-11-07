//! Updater tests: version and asset arithmetic plus the anti-downgrade
//! gate. They exercise pure functions only, so they run on every host.

use super::*;

#[test]
fn asset_names_match_the_release_matrix() {
    assert_eq!(
        asset_candidates("linux", "x86_64").expect("linux"),
        vec!["orquestra-linux-x86_64", "orquestra-ai"]
    );
    assert_eq!(
        asset_candidates("macos", "aarch64").expect("macos"),
        vec!["orquestra-macos-arm64"]
    );
    assert_eq!(
        asset_candidates("macos", "x86_64").expect("macos intel"),
        vec!["orquestra-macos-x86_64"]
    );
    assert_eq!(
        asset_candidates("linux", "aarch64").expect("linux arm64"),
        vec!["orquestra-linux-aarch64"]
    );
    // Windows updates replace the executable in place, exactly like the Unix
    // rows: publishing an MSI is a first-install channel, not an update one.
    assert_eq!(
        asset_candidates("windows", "x86_64").expect("windows"),
        vec!["orquestra-windows-x86_64.exe"]
    );
    // Only x86_64 is published; never guess an asset for an unpublished arch.
    assert!(asset_candidates("windows", "aarch64").is_err());
    assert!(asset_candidates("windows", "i686").is_err());
    assert!(asset_candidates("freebsd", "x86_64").is_err());
}

#[test]
fn asset_url_pins_the_validated_release_tag() {
    let version = validate_version("v0.1.0-alpha.3").expect("tag");
    let binary = release_asset_url(&version, "orquestra-linux-x86_64");
    assert_eq!(
        binary,
        "https://github.com/internetdozero/OrquestraAI/releases/download/v0.1.0-alpha.3/orquestra-linux-x86_64"
    );
    assert!(!binary.contains("/releases/latest/"));
    let sums = release_asset_url(&version, "SHA256SUMS.txt");
    assert!(sums.ends_with("/releases/download/v0.1.0-alpha.3/SHA256SUMS.txt"));
}

#[test]
fn versions_reject_paths_and_blanks() {
    assert_eq!(validate_version("v0.1.0").expect("tag"), "0.1.0");
    assert_eq!(
        validate_version("0.1.0-alpha.2").expect("pre"),
        "0.1.0-alpha.2"
    );
    for hostile in ["", "   ", "../escape", "a/b", "a\\b", "v with space"] {
        assert!(
            validate_version(hostile).is_err(),
            "{hostile:?} must be refused"
        );
    }
}

/// Task #325: equal and lower versions fail closed before any network
/// access; prerelease ordering is numeric (`alpha.5 < alpha.10`).
#[test]
fn downgrades_and_replays_are_refused() {
    let current = env!("CARGO_PKG_VERSION");
    assert!(
        require_newer_version(current).is_err(),
        "re-applying the running version must be refused"
    );
    for older in ["0.1.0-alpha.4", "0.1.0-alpha.1", "0.0.9"] {
        let error = require_newer_version(older).expect_err(&format!("{older} must be refused"));
        assert!(error.contains("not newer"), "says why: {error}");
    }
    // The gate relies on numeric prerelease ordering, not string order.
    let v5 = semver::Version::parse("0.1.0-alpha.5").expect("v5");
    let v10 = semver::Version::parse("0.1.0-alpha.10").expect("v10");
    assert!(v5 < v10, "alpha.5 < alpha.10");
    assert!(require_newer_version("not-a-version").is_err());
    assert!(require_newer_version("").is_err());
}

#[test]
fn checksum_parsing_is_exact_and_unambiguous() {
    let dir = crate::paths::fresh_scratch_dir("orquestra-updater-test").expect("scratch");
    let sums = dir.join("SHA256SUMS.txt");
    fs::write(&sums, "abc123  orquestra-linux-x86_64\n Antenna\n").expect("write");
    assert_eq!(
        expected_hash(&sums, "orquestra-linux-x86_64").expect("hash"),
        "abc123"
    );
    assert!(expected_hash(&sums, "missing-binary").is_err());
    fs::write(
        &sums,
        "abc123  orquestra-linux-x86_64\ndef456 *orquestra-linux-x86_64\n",
    )
    .expect("rewrite");
    assert!(expected_hash(&sums, "orquestra-linux-x86_64").is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sha256_of_matches_the_nist_vector_without_external_tools() {
    let dir = crate::paths::fresh_scratch_dir("orquestra-updater-hash").expect("scratch");
    let file = dir.join("input.bin");
    fs::write(&file, b"abc").expect("write");
    assert_eq!(
        sha256_of(&file).expect("hash"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(sha256_of(&dir.join("missing.bin")).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn staging_uses_the_secure_runtime_root() {
    let staging = Staging::fresh("orquestra-update-test").expect("staging");
    let root = crate::paths::ensure_scratch_root().expect("runtime root");
    assert_eq!(root, crate::paths::scratch_root());
    assert!(staging.0.starts_with(&root));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    // `Staging` removes the tree on drop.
}
