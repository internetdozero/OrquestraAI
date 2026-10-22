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
//! but rejected to keep the change surgical: the existing curl + SHA-256 +
//! atomic-replace flow in `updater.rs` stays untouched apart from this gate.

/// Asset name of the detached signature published next to `SHA256SUMS.txt`.
pub const SUMS_SIGNATURE_ASSET: &str = "SHA256SUMS.txt.minisig";

/// Embedded release public key (minisign `.pub` format). The matching secret
/// key lives only in the `UPDATE_SIGNING_SEED` GitHub Actions secret and the
/// maintainer's offline copy — never in this repo.
const RELEASE_PUBLIC_KEY: &str = "untrusted comment: orquestra release signing key\nRWRXcQWxu51A3hiNwiOi7zV6Dm+uUXR4zZrg1IbsZqD8RAqHZwgKbeU1";

/// Verify a detached minisign signature over the exact `SHA256SUMS.txt` bytes.
/// Prehashed mode only: the release signer always emits modern signatures, so
/// accepting legacy mode would widen the attack surface for no benefit.
/// Public so integration tests can cover the updater's signature gate
/// without network access; production calls flow through `updater.rs`.
pub fn verify_release_file(sums_bytes: &[u8], signature_text: &str) -> Result<(), String> {
    verify_with_key(RELEASE_PUBLIC_KEY, sums_bytes, signature_text)
}

fn verify_with_key(
    public_key_text: &str,
    sums_bytes: &[u8],
    signature_text: &str,
) -> Result<(), String> {
    let public_key = minisign_verify::PublicKey::decode(public_key_text)
        .map_err(|error| format!("update signing key is corrupt: {error:?}"))?;
    let signature = minisign_verify::Signature::decode(signature_text.trim())
        .map_err(|_| "update signature is malformed".to_string())?;
    public_key
        .verify(sums_bytes, &signature, false)
        .map_err(|_| "update signature invalid: refusing an unsigned release".to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// Throwaway test key — NEVER the release key. Its seed is a fixed public
    /// fixture (`b"ORQUESTRA-TEST-KEY-v1-0000000000"`); vectors below were
    /// produced offline with PyNaCl in stock minisign prehashed mode.
    const TEST_PUBLIC_KEY: &str = "untrusted comment: orquestra test release key\nRWQBAgMEBQYHCHD4dL1cWtvSBFcWvyLCYMLOty7K7jKwKWNhmGmVzLR5";
    const SUMS_LEGIT: &str =
        "fd9bf630dd91899266fa7bbf05f069a832bcd52bea1841ab5770a76db076712b  orquestra-linux-x86_64\n";
    const SIG_LEGIT: &str = "untrusted comment: signature from orquestra test release key\nRUQBAgMEBQYHCOvNknYMLfOEIdhrsgmccfoicEqYitSA8dmbpl9D6+yKiWcYU8IB2tlppo29q90cpuh3IFgRtyoW7PwbJ+XSEA8=\ntrusted comment: timestamp=1700000000\tfile=SHA256SUMS.txt\nbfL/M7JLtFxdnhSK5p1dy3q0D0n2EyY+YQro0Q7ux+1ZeQ7CKScCnP7iN5QHYL/Q2C/gok4fwcKh4aoK00I8BA==";
    /// Forged checksums matching a tampered binary (valid SHA-256, no key).
    const SUMS_ATTACKER: &str =
        "6586fbdaf0b6d56dad3e94a49c2515abc2573df65d84fea3dbc73dc226d37d1a  orquestra-linux-x86_64\n";

    #[test]
    fn valid_signature_accepts() {
        verify_with_key(TEST_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), SIG_LEGIT)
            .expect("genuine prehashed signature must verify");
    }

    #[test]
    fn tampered_sums_with_original_signature_reject() {
        // The supply-chain attack: forged binary + matching forged checksums.
        // SHA-256 alone would pass; the signature must still fail closed.
        assert!(verify_with_key(TEST_PUBLIC_KEY, SUMS_ATTACKER.as_bytes(), SIG_LEGIT).is_err());
    }

    #[test]
    fn malformed_signatures_reject() {
        assert!(
            verify_with_key(TEST_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), "not a signature").is_err()
        );
        let truncated = SIG_LEGIT.lines().take(2).collect::<Vec<_>>().join("\n");
        assert!(verify_with_key(TEST_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), &truncated).is_err());
        assert!(verify_with_key(TEST_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), "").is_err());
    }

    #[test]
    fn wrong_key_rejects() {
        // Cross-key check also proves the embedded release key parses.
        assert!(verify_with_key(RELEASE_PUBLIC_KEY, SUMS_LEGIT.as_bytes(), SIG_LEGIT).is_err());
    }

    #[test]
    fn embedded_release_key_parses() {
        minisign_verify::PublicKey::decode(RELEASE_PUBLIC_KEY).expect("release pubkey");
    }
}
