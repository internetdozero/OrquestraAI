//! Integration coverage for the updater's fail-closed flow: the minisign
//! signature gate over `SHA256SUMS.txt` and version validation that runs
//! before any network access. The vectors below use the documented
//! throwaway test key from `update_sig`'s unit tests — never the release
//! key — so they prove the gate logic, not the production trust root.
//
// No `common` import: signature verification and version validation are
// pure and never read `HOME`, so no isolation is needed here.

use orquestra_ai_lib::update_sig::verify_release_file;

/// Throwaway test vectors (key seed `b"ORQUESTRA-TEST-KEY-v1-0000000000"`,
/// produced offline with PyNaCl): valid prehashed minisign material, but
/// signed by a key that is NOT the embedded release key.
const SUMS_LEGIT: &str =
    "fd9bf630dd91899266fa7bbf05f069a832bcd52bea1841ab5770a76db076712b  orquestra-linux-x86_64\n";
const SIG_LEGIT: &str = "untrusted comment: signature from orquestra test release key\nRUQBAgMEBQYHCOvNknYMLfOEIdhrsgmccfoicEqYitSA8dmbpl9D6+yKiWcYU8IB2tlppo29q90cpuh3IFgRtyoW7PwbJ+XSEA8=\ntrusted comment: timestamp=1700000000\tfile=SHA256SUMS.txt\nbfL/M7JLtFxdnhSK5p1dy3q0D0n2EyY+YQro0Q7ux+1ZeQ7CKScCnP7iN5QHYL/Q2C/gok4fwcKh4aoK00I8BA==";
/// Forged checksums matching a tampered binary: valid SHA-256 hex, no key.
const SUMS_ATTACKER: &str =
    "6586fbdaf0b6d56dad3e94a49c2515abc2573df65d84fea3dbc73dc226d37d1a  orquestra-linux-x86_64\n";

#[test]
fn foreign_signature_rejected() {
    // Well-formed and correctly signed — but by the throwaway test key,
    // not the embedded release key. The gate must refuse it, proving it is
    // wired to the production trust root and not merely well-formedness.
    // (Genuine-accept is covered by `update_sig` unit tests, which can
    // inject keys; no valid release signature fixture ships in this repo.)
    assert!(verify_release_file(SUMS_LEGIT.as_bytes(), SIG_LEGIT).is_err());
}

#[test]
fn forged_checksums_fail_closed() {
    // The supply-chain attack: forged binary + matching forged checksums
    // would pass a hash-only check; the signature gate must still refuse.
    assert!(verify_release_file(SUMS_ATTACKER.as_bytes(), SIG_LEGIT).is_err());
}

#[test]
fn malformed_signatures_fail_closed() {
    assert!(verify_release_file(SUMS_LEGIT.as_bytes(), "not a signature").is_err());
    assert!(verify_release_file(SUMS_LEGIT.as_bytes(), "").is_err());
    let truncated = SIG_LEGIT.lines().take(2).collect::<Vec<_>>().join("\n");
    assert!(verify_release_file(SUMS_LEGIT.as_bytes(), &truncated).is_err());
}

#[test]
fn hostile_versions_rejected_before_network() {
    // Validation runs before any curl: no download is attempted, so this
    // proves the fail-closed entry without network access.
    for hostile in ["", "../escape", "a/b", "x with space"] {
        assert!(
            orquestra_ai_lib::apply_app_update(hostile.to_string()).is_err(),
            "{hostile:?} must be refused"
        );
    }
}
