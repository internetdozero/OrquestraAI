//! Encryption at rest for the secret-bearing files.
//!
//! `vault.json` and `mcps.json` carry API tokens, so they are sealed with
//! AES-256-GCM under a per-install file key. The key lives in the OS
//! keyring (Secret Service / Keychain / Credential Manager); headless hosts
//! without a keyring fall back to a `0600` key file with a loud warning.
//! POSIX `0600` on the files themselves stays as defense in depth.
//!
//! Format detection keeps the rollout reversible: [`open_text`] accepts both
//! sealed envelopes and legacy plaintext. On a legacy read the caller backs
//! the plaintext up (`<file>.migrate-bak`) and rewrites it sealed via
//! [`migrate_legacy_file`], so one load migrates the file and dual-read
//! covers crashes in between.

mod envelope;
mod key;

use std::path::Path;

use crate::paths::OrquestraPaths;

/// Seal JSON text for disk. The returned envelope is JSON text, so the
/// existing atomic writers (`write_private`, `write_config`) are unchanged.
pub(crate) fn seal_text(paths: &OrquestraPaths, plaintext: &str) -> Result<String, String> {
    let key = key::file_key(paths)?;
    envelope::seal(&key, plaintext.as_bytes())
}

/// Open stored file text. Returns the plaintext plus whether it was legacy
/// (unsealed) input that the caller should migrate.
pub(crate) fn open_text(paths: &OrquestraPaths, stored: &str) -> Result<(String, bool), String> {
    let value: serde_json::Value = match serde_json::from_str(stored) {
        Ok(value) => value,
        // Not JSON at all: leave it legacy so the caller's parser reports
        // the corruption exactly as before.
        Err(_) => return Ok((stored.to_string(), true)),
    };
    if !envelope::is_sealed(&value) {
        return Ok((stored.to_string(), true));
    }
    let key = key::file_key(paths)?;
    let plaintext = envelope::open(&key, &value)?;
    String::from_utf8(plaintext)
        .map_err(|error| format!("unsealed secrets are not valid text: {error}"))
        .map(|text| (text, false))
}

/// Back up legacy `path` to `<path>.migrate-bak` and return the sealed
/// replacement. The caller persists the sealed text with its own atomic
/// writer; if that write fails the original is untouched and still loads.
pub(crate) fn migrate_legacy_file(
    path: &Path,
    legacy: &str,
    paths: &OrquestraPaths,
) -> Result<String, String> {
    let sealed = seal_text(paths, legacy)?;
    let backup = path.with_extension(format!(
        "{}.migrate-bak",
        path.extension()
            .map(|extension| extension.to_string_lossy().into_owned())
            .unwrap_or_else(|| "json".to_string())
    ));
    std::fs::write(&backup, legacy)
        .map_err(|error| format!("failed to back up {}: {error}", backup.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&backup, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to lock down {}: {error}", backup.display()))?;
    }
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(name: &str) -> OrquestraPaths {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "orquestra-migrate-{name}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("scratch");
        OrquestraPaths {
            root: root.clone(),
            vault: root.clone(),
            active: root,
        }
    }

    #[test]
    fn sealed_text_opens_clean_and_legacy_is_flagged() {
        let paths = test_paths("flag");
        let legacy = r#"{"services":{"a":{"keys":[]}}}"#;
        let (text, was_legacy) = open_text(&paths, legacy).expect("open legacy");
        assert!(was_legacy);
        assert_eq!(text, legacy);
        let sealed = seal_text(&paths, legacy).expect("seal");
        let (reopened, was_legacy) = open_text(&paths, &sealed).expect("open sealed");
        assert!(!was_legacy);
        assert_eq!(reopened, legacy);
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn migration_keeps_a_plaintext_backup_and_round_trips() {
        let paths = test_paths("migrate");
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"github":{"keys":[{"secret":"sk-hidden"}]}}}"#;
        let sealed = migrate_legacy_file(&target, legacy, &paths).expect("migrate");
        assert!(!sealed.contains("sk-hidden"));
        let backup = paths.root.join("vault.json.migrate-bak");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), legacy);
        let (reopened, was_legacy) = open_text(&paths, &sealed).expect("open");
        assert!(!was_legacy);
        assert_eq!(reopened, legacy);
        let _ = std::fs::remove_dir_all(&paths.root);
    }
}
