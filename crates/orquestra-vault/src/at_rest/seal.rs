//! Sealing of secret payloads: the write key plus the seal itself.

use orquestra_core::paths::OrquestraPaths;

use super::envelope;
use super::key;

/// Seal JSON text for disk under the file key. The returned envelope is JSON
/// text, so the existing atomic writers are unchanged.
pub fn seal_text(paths: &OrquestraPaths, plaintext: &str, aad: &[u8]) -> Result<String, String> {
    let key = key::file_key(paths)?;
    seal_with_key(&key, plaintext.as_bytes(), aad)
}

/// Seal with an explicit key (migration re-seals without re-resolving).
pub(super) fn seal_with_key(
    key: &[u8; 32],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<String, String> {
    envelope::seal(key, plaintext, aad)
}
