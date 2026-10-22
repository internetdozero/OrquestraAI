//! AES-256-GCM envelope for secrets at rest.
//!
//! `seal` turns file bytes into a JSON envelope; `open` reverses it.
//! Anything that is not a sealed envelope is reported as legacy plaintext,
//! so pre-encryption `vault.json` / `mcps.json` files keep loading — the
//! callers migrate them (backup + rewrite) on first read.
//!
//! Envelope versions:
//! - `1` (alpha.5): no Additional Authenticated Data. Still accepted so
//!   sealed alpha.5 files keep opening; callers re-seal them as v2.
//! - `2`: binds the payload domain (`vault.v1` / `mcps.v1`) as AES-GCM AAD,
//!   so a `vault.json` envelope pasted into `mcps.json` (or vice versa)
//!   fails closed instead of decrypting under the shared file key.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::prelude::*;

const MARKER: &str = "orquestraSealed";
const VERSION: u8 = 2;
const PREV_VERSION: u8 = 1;
const ALG: &str = "AES-256-GCM";
const NONCE_LEN: usize = 12;

/// Payload domain for the credential vault (`vault.json`).
pub(crate) const AAD_VAULT: &[u8] = b"vault.v1";
/// Payload domain for the MCP catalog (`mcps.json`).
pub(crate) const AAD_MCPS: &[u8] = b"mcps.v1";

/// True when the parsed file value is one of our sealed envelopes (v1 or
/// v2). Legacy files (`services` / `mcps` at the top level) never carry
/// the marker.
pub(super) fn is_sealed(value: &serde_json::Value) -> bool {
    value
        .get(MARKER)
        .and_then(|marker| marker.as_u64())
        .is_some_and(|marker| marker == u64::from(VERSION) || marker == u64::from(PREV_VERSION))
}

/// Envelope version carried by a sealed value, if any.
pub(super) fn sealed_version(value: &serde_json::Value) -> Option<u8> {
    value
        .get(MARKER)
        .and_then(|marker| marker.as_u64())
        .and_then(|marker| u8::try_from(marker).ok())
        .filter(|version| *version == VERSION || *version == PREV_VERSION)
}

/// Encrypt `plaintext` under `key`, binding `aad` (the payload domain),
/// returning the envelope as JSON text (always v2).
pub(super) fn seal(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|error| format!("rejected the file key: {error}"))?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|error| format!("failed to seal secrets: {error}"))?;
    let envelope = serde_json::json!({
        MARKER: VERSION,
        "alg": ALG,
        "nonce": BASE64_STANDARD.encode(nonce.as_slice()),
        "data": BASE64_STANDARD.encode(ciphertext),
    });
    serde_json::to_string_pretty(&envelope)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| format!("failed to serialize the sealed envelope: {error}"))
}

/// Reverse [`seal`], checking the payload domain for v2 envelopes. A wrong
/// key, a swapped domain (`vault` ↔ `mcps`), or a single flipped byte fails
/// closed. Returns the plaintext plus the envelope version so callers can
/// re-seal v1 envelopes as v2.
///
/// v1 envelopes (alpha.5) carry no AAD and are opened without it; anything
/// else is refused.
pub(super) fn open(
    key: &[u8; 32],
    envelope: &serde_json::Value,
    aad: &[u8],
) -> Result<(Vec<u8>, u8), String> {
    let version = sealed_version(envelope)
        .ok_or_else(|| "unsupported sealed envelope version".to_string())?;
    let alg = envelope
        .get("alg")
        .and_then(|alg| alg.as_str())
        .unwrap_or("?");
    if alg != ALG {
        return Err(format!("unsupported sealed envelope algorithm: {alg}"));
    }
    let nonce_bytes = envelope
        .get("nonce")
        .and_then(|nonce| nonce.as_str())
        .ok_or_else(|| "sealed envelope has no nonce".to_string())
        .and_then(|nonce| {
            BASE64_STANDARD
                .decode(nonce)
                .map_err(|error| format!("sealed envelope has a bad nonce: {error}"))
        })?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err("sealed envelope has a bad nonce length".to_string());
    }
    let ciphertext = envelope
        .get("data")
        .and_then(|data| data.as_str())
        .ok_or_else(|| "sealed envelope has no data".to_string())
        .and_then(|data| {
            BASE64_STANDARD
                .decode(data)
                .map_err(|error| format!("sealed envelope has bad data: {error}"))
        })?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|error| format!("rejected the file key: {error}"))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = if version == PREV_VERSION {
        cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| "failed to unseal secrets: wrong key or corrupted file".to_string())?
    } else {
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext.as_ref(),
                    aad,
                },
            )
            .map_err(|_| "failed to unseal secrets: wrong key or corrupted file".to_string())?
    };
    Ok((plaintext, version))
}

/// Craft a v1 (alpha.5, no-AAD) envelope for upgrade tests.
#[cfg(test)]
pub(crate) fn seal_v1_for_tests(key: &[u8; 32], plaintext: &[u8]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|error| format!("rejected the file key: {error}"))?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|error| format!("failed to seal secrets: {error}"))?;
    let envelope = serde_json::json!({
        MARKER: PREV_VERSION,
        "alg": ALG,
        "nonce": BASE64_STANDARD.encode(nonce.as_slice()),
        "data": BASE64_STANDARD.encode(ciphertext),
    });
    serde_json::to_string_pretty(&envelope)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| format!("failed to serialize the sealed envelope: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        Aes256Gcm::generate_key(&mut OsRng).into()
    }

    #[test]
    fn round_trip_restores_the_exact_bytes() {
        let key = test_key();
        let plaintext = br#"{"services":{"github":{"keys":[{"secret":"sk-live"}]}}}"#;
        let sealed = seal(&key, plaintext, AAD_VAULT).expect("seal");
        assert!(!sealed.contains("sk-live"));
        let value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope is JSON");
        assert!(is_sealed(&value));
        assert_eq!(sealed_version(&value), Some(2));
        let (reopened, version) = open(&key, &value, AAD_VAULT).expect("open");
        assert_eq!(version, 2);
        assert_eq!(reopened, plaintext);
    }

    #[test]
    fn tampered_ciphertext_fails_closed() {
        let key = test_key();
        let sealed = seal(&key, b"top-secret", AAD_VAULT).expect("seal");
        let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("JSON");
        let data = value.get("data").unwrap().as_str().unwrap().to_string();
        let mut raw = BASE64_STANDARD.decode(&data).unwrap();
        raw[0] ^= 0x01;
        value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
        assert!(open(&key, &value, AAD_VAULT).is_err());
    }

    #[test]
    fn wrong_key_fails_closed() {
        let key1 = test_key();
        let key2 = test_key();
        let sealed = seal(&key1, b"top-secret", AAD_VAULT).expect("seal");
        let value: serde_json::Value = serde_json::from_str(&sealed).expect("JSON");
        assert!(open(&key2, &value, AAD_VAULT).is_err());
    }

    #[test]
    fn legacy_vault_shape_is_not_mistaken_for_sealed() {
        let legacy: serde_json::Value =
            serde_json::from_str(r#"{"services":{},"other":1}"#).expect("JSON");
        assert!(!is_sealed(&legacy));
        assert_eq!(sealed_version(&legacy), None);
        let garbage = serde_json::Value::String("not-json-at-all".to_string());
        assert!(!is_sealed(&garbage));
    }

    #[test]
    fn swapped_domains_are_rejected() {
        let key = test_key();
        let sealed = seal(&key, b"vault-plaintext", AAD_VAULT).expect("seal");
        let value: serde_json::Value = serde_json::from_str(&sealed).expect("JSON");
        assert!(open(&key, &value, AAD_VAULT).is_ok());
        assert!(
            open(&key, &value, AAD_MCPS).is_err(),
            "a vault envelope must not open under the mcps domain"
        );
        let sealed_mcps = seal(&key, b"mcps-plaintext", AAD_MCPS).expect("seal mcps");
        let value_mcps: serde_json::Value = serde_json::from_str(&sealed_mcps).expect("JSON");
        assert!(open(&key, &value_mcps, AAD_VAULT).is_err());
    }
}
