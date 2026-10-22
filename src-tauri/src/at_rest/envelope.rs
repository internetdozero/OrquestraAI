//! AES-256-GCM envelope for secrets at rest.
//!
//! `seal` turns file bytes into a JSON envelope; `open` reverses it.
//! Anything that is not a sealed envelope is reported as legacy plaintext,
//! so pre-encryption `vault.json` / `mcps.json` files keep loading — the
//! callers migrate them (backup + rewrite) on first read.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::prelude::*;

const MARKER: &str = "orquestraSealed";
const VERSION: u8 = 1;
const ALG: &str = "AES-256-GCM";
const NONCE_LEN: usize = 12;

/// True when the parsed file value is one of our sealed envelopes. Legacy
/// files (`services` / `mcps` at the top level) never carry the marker.
pub(super) fn is_sealed(value: &serde_json::Value) -> bool {
    value
        .get(MARKER)
        .and_then(|marker| marker.as_u64())
        .is_some_and(|marker| marker == u64::from(VERSION))
}

/// Encrypt `plaintext` under `key`, returning the envelope as JSON text.
pub(super) fn seal(key: &[u8; 32], plaintext: &[u8]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|error| format!("rejected the file key: {error}"))?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
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

/// Reverse [`seal`]. A wrong key or a single flipped byte fails closed.
pub(super) fn open(key: &[u8; 32], envelope: &serde_json::Value) -> Result<Vec<u8>, String> {
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
    cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext.as_ref())
        .map_err(|_| "failed to unseal secrets: wrong key or corrupted file".to_string())
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
        let sealed = seal(&key, plaintext).expect("seal");
        assert!(!sealed.contains("sk-live"));
        let value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope is JSON");
        assert!(is_sealed(&value));
        assert_eq!(open(&key, &value).expect("open"), plaintext);
    }

    #[test]
    fn tampered_ciphertext_fails_closed() {
        let key = test_key();
        let sealed = seal(&key, b"top-secret").expect("seal");
        let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("JSON");
        let data = value.get("data").unwrap().as_str().unwrap().to_string();
        let mut raw = BASE64_STANDARD.decode(&data).unwrap();
        raw[0] ^= 0x01;
        value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
        assert!(open(&key, &value).is_err());
    }

    #[test]
    fn wrong_key_fails_closed() {
        let key1 = test_key();
        let key2 = test_key();
        let sealed = seal(&key1, b"top-secret").expect("seal");
        let value: serde_json::Value = serde_json::from_str(&sealed).expect("JSON");
        assert!(open(&key2, &value).is_err());
    }

    #[test]
    fn legacy_vault_shape_is_not_mistaken_for_sealed() {
        let legacy: serde_json::Value =
            serde_json::from_str(r#"{"services":{},"other":1}"#).expect("JSON");
        assert!(!is_sealed(&legacy));
        let garbage = serde_json::Value::String("not-json-at-all".to_string());
        assert!(!is_sealed(&garbage));
    }
}
