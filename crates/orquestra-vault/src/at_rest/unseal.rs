//! Opening sealed payloads: read path, refusal of plaintext, reopen
//! verification, and the stranded-key fallbacks.

use std::path::Path;

use orquestra_core::paths::OrquestraPaths;

use super::backup::startup_cleanup;
use super::envelope;
use super::io::persist_sealed;
use super::key;
use super::migrate::{migrate_legacy_and_verify, self_heal, self_heal_no_key};
use super::seal::seal_with_key;
use super::{AAD_MCPS, AAD_VAULT};

/// Open a stored file's text with the full migration flow (Task #323).
/// Returns the plaintext plus whether it was legacy input.
///
/// - Legacy JSON (or non-JSON) plaintext: refused once the migrated flag is
///   set; otherwise backed up `0600`, sealed, verified, and unlinked.
/// - Sealed v1: opened without AAD, then re-sealed as v2.
/// - Sealed v2: opened with the caller's AAD (swapped domains fail).
/// - Key fallback: the keyring key is tried first, then `.file-key` (reads
///   never create key material). A fallback success promotes its bytes into
///   the keyring and re-seals under the active key; `.file-key` is removed
///   only after the re-sealed files verify.
/// - Unseal failure: self-heals from the `.migrate-bak` backup (plaintext
///   or sealed) when it holds valid JSON, re-sealing under the current key.
pub fn open_stored(
    paths: &OrquestraPaths,
    path: &Path,
    stored: &str,
    aad: &[u8],
) -> Result<(String, bool), String> {
    let _ = startup_cleanup(paths);
    let value: serde_json::Value = match serde_json::from_str(stored) {
        Ok(value) => value,
        Err(_) => {
            if key::is_migrated(&paths.root) {
                return Err(refuse_plaintext(path));
            }
            return Ok((stored.to_string(), true));
        }
    };
    if !envelope::is_sealed(&value) {
        if key::is_migrated(&paths.root) {
            return Err(refuse_plaintext(path));
        }
        migrate_legacy_and_verify(paths, path, stored, aad)?;
        return Ok((stored.to_string(), true));
    }
    // Sealed: keyring first, then fallback. Neither read creates keys.
    let mut keyring_err: Option<String> = None;
    if key::keyring_usable(&paths.root) {
        match key::read_keyring_key() {
            Ok(Some(kp)) => match envelope::open(&kp, &value, aad) {
                Ok((plaintext, version)) => {
                    key::cache_insert(&paths.root, kp);
                    let text = String::from_utf8(plaintext)
                        .map_err(|error| format!("unsealed secrets are not valid text: {error}"))?;
                    if version == 1 {
                        let resealed = seal_with_key(&kp, text.as_bytes(), aad)?;
                        persist_sealed(path, &resealed)?;
                        verify_reopen(paths, path, aad, Some(&text))?;
                        key::mark_migrated(&paths.root)?;
                    } else {
                        let _ = maybe_remove_fallback(paths);
                    }
                    return Ok((text, false));
                }
                Err(error) => keyring_err = Some(error),
            },
            Ok(None) => {}
            Err(error) => {
                key::warn_once(format!(
                    "OS keyring unreachable ({error}); trying the fallback key file"
                ));
                keyring_err = Some(error);
            }
        }
    }
    match key::read_fallback_key(&paths.root)? {
        Some(kf) => match envelope::open(&kf, &value, aad) {
            Ok((plaintext, version)) => {
                let text = String::from_utf8(plaintext)
                    .map_err(|error| format!("unsealed secrets are not valid text: {error}"))?;
                if key::keyring_usable(&paths.root) {
                    let active = key::promote_fallback_to_keyring(&kf)?;
                    key::cache_insert(&paths.root, active);
                    let resealed = seal_with_key(&active, text.as_bytes(), aad)?;
                    persist_sealed(path, &resealed)?;
                    verify_reopen(paths, path, aad, Some(&text))?;
                    let _ = maybe_remove_fallback(paths);
                    key::mark_migrated(&paths.root)?;
                } else if version == 1 {
                    // Headless host with no keyring: no promotion target, but
                    // the v1 → v2 upgrade still applies under the same key.
                    key::cache_insert(&paths.root, kf);
                    let resealed = seal_with_key(&kf, text.as_bytes(), aad)?;
                    persist_sealed(path, &resealed)?;
                    verify_reopen(paths, path, aad, Some(&text))?;
                    key::mark_migrated(&paths.root)?;
                } else {
                    key::cache_insert(&paths.root, kf);
                }
                Ok((text, false))
            }
            Err(fallback_err) => self_heal(paths, path, aad, keyring_err, fallback_err),
        },
        None => {
            let err = keyring_err.unwrap_or_else(|| "no file key available".to_string());
            self_heal_no_key(paths, path, aad, err)
        }
    }
}

fn refuse_plaintext(path: &Path) -> String {
    format!(
        "refusing to read {}: plaintext secrets are no longer accepted after the \
         encryption migration completed — restore the sealed file or rotate the \
         exposed keys (see CHANGELOG.md)",
        path.display()
    )
}

/// Reopen `path` under the available keys (never creating any) and, when
/// `expected` is given, compare byte for byte.
pub(super) fn verify_reopen(
    paths: &OrquestraPaths,
    path: &Path,
    aad: &[u8],
    expected: Option<&str>,
) -> Result<(), String> {
    let stored = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to re-read {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&stored)
        .map_err(|error| format!("re-sealed {} does not parse: {error}", path.display()))?;
    if !envelope::is_sealed(&value) {
        return Err(format!(
            "re-sealed {} is not a sealed envelope",
            path.display()
        ));
    }
    let text = try_open_with_available_keys(paths, &value, aad)?;
    if let Some(want) = expected {
        if text != want {
            return Err(format!(
                "re-sealed {} does not match the migrated plaintext",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Try the keyring key, then the fallback key. Reads only — never creates.
pub(super) fn try_open_with_available_keys(
    paths: &OrquestraPaths,
    envelope_value: &serde_json::Value,
    aad: &[u8],
) -> Result<String, String> {
    if key::keyring_usable(&paths.root) {
        if let Ok(Some(kp)) = key::read_keyring_key() {
            if let Ok((plaintext, _)) = envelope::open(&kp, envelope_value, aad) {
                return String::from_utf8(plaintext)
                    .map_err(|error| format!("unsealed secrets are not valid text: {error}"));
            }
        }
    }
    if let Some(kf) = key::read_fallback_key(&paths.root)? {
        if let Ok((plaintext, _)) = envelope::open(&kf, envelope_value, aad) {
            return String::from_utf8(plaintext)
                .map_err(|error| format!("unsealed secrets are not valid text: {error}"));
        }
    }
    Err("failed to unseal: wrong key or corrupted file".to_string())
}

/// After a key promotion, drop `.file-key` once every existing sealed file
/// opens under the keyring key. Returns `true` when the fallback is gone.
fn maybe_remove_fallback(paths: &OrquestraPaths) -> Result<bool, String> {
    if !key::fallback_exists(&paths.root) || !key::keyring_usable(&paths.root) {
        return Ok(false);
    }
    let Ok(Some(kp)) = key::read_keyring_key() else {
        return Ok(false);
    };
    for (name, aad) in [("vault.json", AAD_VAULT), ("mcps.json", AAD_MCPS)] {
        let path = paths.root.join(name);
        let Ok(stored) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&stored) else {
            continue;
        };
        if !envelope::is_sealed(&value) {
            continue;
        }
        if envelope::open(&kp, &value, aad).is_err() {
            return Ok(false);
        }
    }
    key::remove_fallback_key(&paths.root)
}
