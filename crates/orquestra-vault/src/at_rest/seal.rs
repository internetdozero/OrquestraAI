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
///
/// Refuses input that is already a sealed envelope: every file write in
/// this module funnels through here, so one guard makes nesting impossible
/// no matter which caller regresses.
pub(super) fn seal_with_key(
    key: &[u8; 32],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<String, String> {
    if envelope::is_envelope_bytes(plaintext) {
        return Err(
            "refusing to seal: input is already a sealed envelope; unwrap it first instead of nesting"
                .to_string(),
        );
    }
    envelope::seal(key, plaintext, aad)
}
