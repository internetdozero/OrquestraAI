//! Supply-chain signature gate for the auto-updater.
//!
//! The release workflow signs the published `SHA256SUMS.txt` with the offline
//! Ed25519 release key and uploads `SHA256SUMS.txt.minisig` next to it (see
//! `scripts/sign-release.py`). The public half is embedded below, so a
//! compromised GitHub Release (forged binary + forged checksums) still fails
//! closed: without the secret key no valid signature can be produced.
//!
//! The format is stock minisign prehashed mode (`ED` algorithm marker), so any
//! signature this module accepts also verifies with the reference
//! `minisign -V` tool. `tauri-plugin-updater`'s native scheme was considered
//! but rejected to keep the change surgical: the existing reqwest + SHA-256 +
//! atomic-replace flow in `updater.rs` stays untouched apart from this gate.

/// Asset name of the detached signature published next to `SHA256SUMS.txt`.
pub const SUMS_SIGNATURE_ASSET: &str = "SHA256SUMS.txt.minisig";

/// Embedded release public key (minisign `.pub` format). The matching secret
/// key lives only in the `UPDATE_SIGNING_SEED` GitHub Actions secret and the
/// maintainer's offline copy — never in this repo.
const RELEASE_PUBLIC_KEY: &str = "untrusted comment: ownhub release signing key\nRWRXcQWxu51A3hiNwiOi7zV6Dm+uUXR4zZrg1IbsZqD8RAqHZwgKbeU1";

/// Verify a detached minisign signature over the exact `SHA256SUMS.txt` bytes
/// and bind it to `expected_version` (the validated update target, no `v`).
/// Prehashed mode only: the release signer always emits modern signatures, so
/// accepting legacy mode would widen the attack surface for no benefit.
/// Public so integration tests can cover the updater's signature gate
/// without network access; production calls flow through `updater.rs`.
///
/// After the cryptographic check, the signature's trusted comment must carry
/// `version=<expected_version>` and `file=SHA256SUMS.txt`, exactly as
/// `scripts/sign-release.py` emits them. A signature lifted from an older
/// release (replay/downgrade) verifies cryptographically but names the wrong
/// version, so it still fails closed here.
pub fn verify_release_file(
    sums_bytes: &[u8],
    signature_text: &str,
    expected_version: &str,
) -> Result<(), String> {
    verify_with_key(
        RELEASE_PUBLIC_KEY,
        sums_bytes,
        signature_text,
        expected_version,
    )
}

fn verify_with_key(
    public_key_text: &str,
    sums_bytes: &[u8],
    signature_text: &str,
    expected_version: &str,
) -> Result<(), String> {
    let public_key = minisign_verify::PublicKey::decode(public_key_text)
        .map_err(|error| format!("update signing key is corrupt: {error:?}"))?;
    let signature = minisign_verify::Signature::decode(signature_text.trim())
        .map_err(|_| "update signature is malformed".to_string())?;
    public_key
        .verify(sums_bytes, &signature, false)
        .map_err(|_| "update signature invalid: refusing an unsigned release".to_string())?;
    require_trusted_comment(&signature, expected_version)
}

/// The anti-replay binding: `version=` must equal the validated target tag
/// and `file=` must equal `SHA256SUMS.txt`. A missing or mismatched value
/// fails closed — the signature may be genuine, but for another release.
fn require_trusted_comment(
    signature: &minisign_verify::Signature,
    expected_version: &str,
) -> Result<(), String> {
    let mut version: Option<&str> = None;
    let mut file: Option<&str> = None;
    for field in signature.trusted_comment().split_whitespace() {
        if let Some(value) = field.strip_prefix("version=") {
            version = Some(value);
        } else if let Some(value) = field.strip_prefix("file=") {
            file = Some(value);
        }
    }
    match (version, file) {
        (Some(actual), Some("SHA256SUMS.txt")) if actual == expected_version => Ok(()),
        (Some(actual), Some("SHA256SUMS.txt")) => Err(format!(
            "update signature is for version {actual:?}, not the validated target {expected_version:?}: refusing a replayed release"
        )),
        _ => Err(format!(
            "update signature trusted comment must bind version={expected_version:?} and file=SHA256SUMS.txt: refusing an unbound release"
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// Throwaway test key — NEVER the release key. Its seed is a fixed public
    /// fixture (`b"ORQUESTRA-TEST-KEY-v1-0000000000"`, kept under the
    /// pre-rename name so the keypair stays stable); vectors below were
    /// produced offline with PyNaCl in stock minisign prehashed mode and
    /// regenerated for the `ownhub-linux-x86_64` asset name.
    const TEST_PUBLIC_KEY: &str = "untrusted comment: ownhub test release key\nRWQBAgMEBQYHCHD4dL1cWtvSBFcWvyLCYMLOty7K7jKwKWNhmGmVzLR5";
    const SUMS_LEGIT: &str =
        "fd9bf630dd91899266fa7bbf05f069a832bcd52bea1841ab5770a76db076712b  ownhub-linux-x86_64\n";
    const SIG_LEGIT: &str = "untrusted comment: signature from ownhub test release key\nRUQBAgMEBQYHCO2RToRzWNRAGxKImbauHteYLKzRi1pW3Vh9uFOo8xryjltKZruzR/M4+x1P/hciZ3jVC28S+800ILp9vsY5hQo=\ntrusted comment: timestamp=1700000000\tfile=SHA256SUMS.txt\ngyUiAgGgG2XAHsrjCH+eHPP2hx+j/FOxlCkbj2b7dqrTczfr94UxIvShuROkPMVPIZUvZYkLGmRtqM8IPGoyCw==";
    /// Forged checksums matching a tampered binary (valid SHA-256, no key).
    const SUMS_ATTACKER: &str =
        "6586fbdaf0b6d56dad3e94a49c2515abc2573df65d84fea3dbc73dc226d37d1a  ownhub-linux-x86_64\n";

    /// Throwaway version-bound test key (seed `b"ORQUESTRA-VERSION-TEST-000000001"`,
    /// keynum `0x02 * 8`, vectors produced offline with PyNaCl): the same
    /// prehashed format `scripts/sign-release.py` emits, including the
    /// `version=` trusted-comment binding.
    const VERSIONED_PUBLIC_KEY: &str = "untrusted comment: ownhub version-bound test release key\nRUQCAgICAgICAk0QGDmXR7u7JJV+Q5P0XjGFpcW6SXmj1cF3TheI4J9O";
    const SIG_V5: &str = "untrusted comment: signature from ownhub version-bound test key\nRUQCAgICAgICAr5ZfDJsKD1mQue0KWuonmNO63BVYod8ZC7sbep4pn2NZI5CrrsLPBkm/tm5/kI+5cEwTxjLEoKaGn3ikME+3gA=\ntrusted comment: timestamp=1700000000\tfile=SHA256SUMS.txt\tversion=0.1.0-alpha.5\nOiK6PIusflIfiJBL3axfLpBFyw2NVBTKmaK/S//mmhLpEKGVzX6UVJ3qDr5xSWuq4L+eoX0x+8ZYdo6yycJWDg==";
    const SIG_V4: &str = "untrusted comment: signature from ownhub version-bound test key\nRUQCAgICAgICAr5ZfDJsKD1mQue0KWuonmNO63BVYod8ZC7sbep4pn2NZI5CrrsLPBkm/tm5/kI+5cEwTxjLEoKaGn3ikME+3gA=\ntrusted comment: timestamp=1700000000\tfile=SHA256SUMS.txt\tversion=0.1.0-alpha.4\nKE4QfDCLt2t8f4BJ4wV6ToceGAmnpg7H/hX3o54ohHLM/akClrMIItWP45u8C/n/bxTDVnAaHAqIt3PnmirBAw==";

    #[test]
    fn valid_signature_accepts() {
        verify_with_key(TEST_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), SIG_LEGIT, "")
            .expect_err("legacy trusted comment without version= must not pass");
        verify_with_key(
            VERSIONED_PUBLIC_KEY,
            SUMS_LEGIT.as_bytes(),
            SIG_V5,
            "0.1.0-alpha.5",
        )
        .expect("version-bound signature must verify");
    }

    #[test]
    fn tampered_sums_with_original_signature_reject() {
        // The supply-chain attack: forged binary + matching forged checksums.
        // SHA-256 alone would pass; the signature must still fail closed.
        assert!(verify_with_key(TEST_PUBLIC_KEY, SUMS_ATTACKER.as_bytes(), SIG_LEGIT, "").is_err());
    }

    #[test]
    fn trusted_comment_version_mismatch_rejects() {
        // Genuine signature, cryptographically valid — but for alpha.4.
        // Replaying it onto an alpha.5 update must fail closed.
        let error = verify_with_key(
            VERSIONED_PUBLIC_KEY,
            SUMS_LEGIT.as_bytes(),
            SIG_V4,
            "0.1.0-alpha.5",
        )
        .expect_err("replayed older signature must be refused");
        assert!(
            error.contains("0.1.0-alpha.4"),
            "names the signed version: {error}"
        );
        assert!(error.contains("replayed"), "says why: {error}");
    }

    #[test]
    fn trusted_comment_missing_version_rejects() {
        // The alpha.5-era format has no `version=` at all: unbound.
        let error = verify_with_key(
            TEST_PUBLIC_KEY,
            SUMS_LEGIT.as_bytes(),
            SIG_LEGIT,
            "0.1.0-alpha.5",
        )
        .expect_err("unbound signature must be refused");
        assert!(error.contains("version="), "says what is missing: {error}");
    }

    #[test]
    fn malformed_signatures_reject() {
        assert!(verify_with_key(
            TEST_PUBLIC_KEY,
            SUMS_LEGIT.as_bytes(),
            "not a signature",
            ""
        )
        .is_err());
        let truncated = SIG_LEGIT.lines().take(2).collect::<Vec<_>>().join("\n");
        assert!(verify_with_key(TEST_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), &truncated, "").is_err());
        assert!(verify_with_key(TEST_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), "", "").is_err());
    }

    #[test]
    fn wrong_key_rejects() {
        // Cross-key check also proves the embedded release key parses.
        assert!(verify_with_key(RELEASE_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), SIG_LEGIT, "").is_err());
    }

    #[test]
    fn embedded_release_key_parses() {
        minisign_verify::PublicKey::decode(RELEASE_PUBLIC_KEY).expect("release pubkey");
    }
}
