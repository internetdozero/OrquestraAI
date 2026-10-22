//! File-level migration tests, kept out of `mod.rs`: they drive the seal,
//! unseal, migration and backup paths through the module's surface.

use orquestra_core::paths::OrquestraPaths;

use super::backup::{
    migrate_backup_path, purge_migration_backup, recover_plaintext_from_backup, startup_cleanup,
};
use super::envelope;
use super::io::{persist_sealed, write_private_new, zeroize_file_contents};
use super::key;
use super::seal::{seal_text, seal_with_key};
use super::unseal::open_stored;
use super::AAD_VAULT;

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
    let (text, was_legacy) = open_stored(&paths, &paths.root.join("vault.json"), legacy, AAD_VAULT)
        .expect("open legacy");
    assert!(was_legacy);
    assert_eq!(text, legacy);
    // The migration sealed the file and left no plaintext backup behind.
    let target = paths.root.join("vault.json");
    let stored = std::fs::read_to_string(&target).expect("sealed file");
    let value: serde_json::Value = serde_json::from_str(&stored).expect("envelope");
    assert!(envelope::is_sealed(&value));
    assert_eq!(envelope::sealed_version(&value), Some(2));
    assert!(!migrate_backup_path(&target).exists());
    let (reopened, was_legacy) =
        open_stored(&paths, &target, &stored, AAD_VAULT).expect("open sealed");
    assert!(!was_legacy);
    assert_eq!(reopened, legacy);
    let _ = std::fs::remove_dir_all(&paths.root);
}

/// Task #314: a sealed file the current key no longer opens (rotation,
/// restored backup, flipped byte) recovers from `.migrate-bak`.
#[test]
fn corrupt_sealed_file_recovers_from_migrate_bak() {
    use base64::prelude::*;
    let paths = test_paths("selfheal");
    let target = paths.root.join("vault.json");
    let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
    let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
    persist_sealed(&target, &sealed).expect("write sealed");
    // One flipped ciphertext byte fails closed under the same key.
    let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope");
    let mut raw = BASE64_STANDARD
        .decode(value.get("data").unwrap().as_str().unwrap())
        .expect("data");
    raw[0] ^= 0x01;
    value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
    let corrupted = serde_json::to_string_pretty(&value).unwrap();
    persist_sealed(&target, &corrupted).expect("corrupt");
    assert!(
        open_stored(&paths, &target, &corrupted, AAD_VAULT).is_err(),
        "tampered seal must fail"
    );
    // No backup yet: nothing to heal from.
    assert!(recover_plaintext_from_backup(&paths, &target, AAD_VAULT).is_none());
    write_private_new(&migrate_backup_path(&target), legacy.as_bytes()).expect("plant backup");
    assert_eq!(
        recover_plaintext_from_backup(&paths, &target, AAD_VAULT).as_deref(),
        Some(legacy)
    );
    let _ = std::fs::remove_dir_all(&paths.root);
}

/// Task #320: purge zeroizes then unlinks; a missing backup is `Ok(false)`.
#[test]
fn purge_zeroizes_then_removes_the_backup() {
    let paths = test_paths("purge");
    let target = paths.root.join("vault.json");
    let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
    let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
    persist_sealed(&target, &sealed).expect("write sealed");
    let backup = migrate_backup_path(&target);
    write_private_new(&backup, legacy.as_bytes()).expect("plant backup");
    assert!(purge_migration_backup(&paths, &target, AAD_VAULT).expect("purge"));
    assert!(!backup.exists(), "backup must be unlinked");
    assert!(
        !purge_migration_backup(&paths, &target, AAD_VAULT).expect("re-purge"),
        "absent backup reports Ok(false)"
    );
    let _ = std::fs::remove_dir_all(&paths.root);
}

/// Task #320: while the sealed file does not open, the backup — possibly
/// the only readable copy — survives the refused purge.
#[test]
fn purge_refuses_while_the_sealed_file_does_not_open() {
    use base64::prelude::*;
    let paths = test_paths("purge-refuse");
    let target = paths.root.join("vault.json");
    let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
    let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
    let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope");
    let mut raw = BASE64_STANDARD
        .decode(value.get("data").unwrap().as_str().unwrap())
        .expect("data");
    raw[0] ^= 0x01;
    value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
    persist_sealed(&target, &serde_json::to_string_pretty(&value).unwrap())
        .expect("corrupt sealed");
    let backup = migrate_backup_path(&target);
    write_private_new(&backup, legacy.as_bytes()).expect("plant backup");
    assert!(purge_migration_backup(&paths, &target, AAD_VAULT).is_err());
    assert_eq!(
        std::fs::read_to_string(&backup).expect("backup survives"),
        legacy
    );
    let _ = std::fs::remove_dir_all(&paths.root);
}

/// Task #320: the zeroize helper blanks every byte past the 64KB chunk
/// boundary without changing the file length.
#[test]
fn zeroize_helper_blanks_every_byte() {
    let paths = test_paths("purge-zero");
    let file = paths.root.join("scratch.bin");
    let original: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&file, &original).expect("write scratch");
    let len = std::fs::metadata(&file).expect("meta").len();
    zeroize_file_contents(&file, len).expect("zeroize");
    let blanked = std::fs::read(&file).expect("read blanked");
    assert_eq!(blanked.len(), original.len());
    assert!(blanked.iter().all(|byte| *byte == 0));
    let _ = std::fs::remove_dir_all(&paths.root);
}

/// Task #323: once the migrated flag is set, plaintext is refused — and
/// every secret file is created `0600`.
#[test]
fn legacy_plaintext_is_refused_once_the_flag_is_set() {
    let paths = test_paths("flag-refuse");
    let target = paths.root.join("vault.json");
    let legacy = r#"{"services":{"github":{"keys":[{"secret":"sk-hidden"}]}}}"#;
    let (text, was_legacy) = open_stored(&paths, &target, legacy, AAD_VAULT).expect("migrate");
    assert!(was_legacy);
    assert_eq!(text, legacy);
    assert!(key::is_migrated(&paths.root), "migration sets the flag");
    assert!(!migrate_backup_path(&target).exists(), "backup unlinked");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // A planted plaintext file is now a downgrade: refused, not re-sealed.
    let stored = std::fs::read_to_string(&target).expect("sealed stands");
    std::fs::write(&target, legacy).expect("plant plaintext");
    let err = open_stored(&paths, &target, legacy, AAD_VAULT).expect_err("must refuse");
    assert!(err.contains("no longer accepted"), "clear refusal: {err}");
    // The sealed file is restored for the next load.
    persist_sealed(&target, &stored).expect("restore sealed");
    let (reopened, was_legacy) =
        open_stored(&paths, &target, &stored, AAD_VAULT).expect("open sealed");
    assert!(!was_legacy);
    assert_eq!(reopened, legacy);
    let _ = std::fs::remove_dir_all(&paths.root);
}

/// Task #323: sealed v1 (alpha.5, no AAD) opens and is re-sealed as v2.
#[test]
fn v1_envelopes_upgrade_to_v2_on_load() {
    let paths = test_paths("v1-upgrade");
    let target = paths.root.join("vault.json");
    let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
    let key = key::file_key(&paths).expect("file key");
    let v1 = envelope::seal_v1_for_tests(&key, legacy.as_bytes()).expect("v1 seal");
    persist_sealed(&target, &v1).expect("plant v1");
    let stored = std::fs::read_to_string(&target).expect("read v1");
    let before: serde_json::Value = serde_json::from_str(&stored).expect("v1 JSON");
    assert_eq!(envelope::sealed_version(&before), Some(1));
    let (text, was_legacy) = open_stored(&paths, &target, &stored, AAD_VAULT).expect("upgrade");
    assert!(!was_legacy);
    assert_eq!(text, legacy);
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&target).expect("re-read")).expect("v2 JSON");
    assert_eq!(envelope::sealed_version(&after), Some(2));
    let (again, _) = open_stored(
        &paths,
        &target,
        &std::fs::read_to_string(&target).expect("read v2"),
        AAD_VAULT,
    )
    .expect("v2 opens");
    assert_eq!(again, legacy);
    let _ = std::fs::remove_dir_all(&paths.root);
}

/// Task #323: SSH → desktop handoff — a vault sealed under the fallback
/// file key promotes its bytes into the (mock) keyring on first unseal,
/// re-seals under the active key, and drops `.file-key` only after the
/// re-sealed file verifies.
#[test]
fn fallback_key_migrates_to_keyring_on_unseal() {
    use base64::prelude::*;
    let _serial = key::test_keyring_lock();
    key::test_keyring_clear();
    // A non-temp root takes the keyring path (temp roots stay pinned to
    // the fallback file so suites never touch a shared key namespace).
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!(
            "orquestra-at-rest-handoff-{}-{nanos}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("scratch");
    let paths = OrquestraPaths {
        root: root.clone(),
        vault: root.clone(),
        active: root.clone(),
    };
    assert!(
        key::keyring_usable(&paths.root),
        "mock keyring must be usable"
    );
    let target = paths.root.join("vault.json");
    let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
    let fallback: [u8; 32] = [0x77; 32];
    // Plant the SSH-era state: fallback file + file sealed under it.
    write_private_new(
        &paths.root.join(".file-key"),
        format!("{}\n", BASE64_STANDARD.encode(fallback)).as_bytes(),
    )
    .expect("plant fallback");
    let sealed = seal_with_key(&fallback, legacy.as_bytes(), AAD_VAULT).expect("seal");
    persist_sealed(&target, &sealed).expect("plant sealed");
    // First unseal promotes, re-seals, and removes the fallback file.
    let stored = std::fs::read_to_string(&target).expect("read sealed");
    let (text, was_legacy) = open_stored(&paths, &target, &stored, AAD_VAULT).expect("unseal");
    assert!(!was_legacy);
    assert_eq!(text, legacy);
    let raw = key::test_keyring_raw().expect("read mock");
    assert_eq!(
        raw.as_deref(),
        Some(BASE64_STANDARD.encode(fallback).as_str()),
        "fallback bytes move into the keyring"
    );
    assert!(
        !paths.root.join(".file-key").exists(),
        ".file-key is removed after the re-sealed file verifies"
    );
    let reopened: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&target).expect("re-read"))
            .expect("envelope");
    assert_eq!(envelope::sealed_version(&reopened), Some(2));
    key::test_keyring_clear();
    let _ = std::fs::remove_dir_all(&paths.root);
}

/// Task #323: `startup_cleanup` removes alpha.5 plaintext backups once
/// the sealed file verifies, and leaves sealed backups for the purge
/// button — as well as backups whose sealed file does not open.
#[test]
fn startup_cleanup_sweeps_only_verified_plaintext_backups() {
    let paths = test_paths("startup-sweep");
    let target = paths.root.join("vault.json");
    let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
    // Verified pair: plaintext backup goes away.
    let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
    persist_sealed(&target, &sealed).expect("write sealed");
    let backup = migrate_backup_path(&target);
    write_private_new(&backup, legacy.as_bytes()).expect("plant plaintext backup");
    assert!(startup_cleanup(&paths).expect("sweep"));
    assert!(!backup.exists());
    // A sealed backup is left for the explicit purge button.
    write_private_new(&backup, sealed.as_bytes()).expect("plant sealed backup");
    assert!(!startup_cleanup(&paths).expect("sweep sealed"));
    assert!(backup.exists());
    let _ = std::fs::remove_file(&backup);
    // An unverifiable sealed file keeps its plaintext backup: it may be
    // the only readable copy.
    use base64::prelude::*;
    let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope");
    let mut raw = BASE64_STANDARD
        .decode(value.get("data").unwrap().as_str().unwrap())
        .expect("data");
    raw[0] ^= 0x01;
    value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
    persist_sealed(&target, &serde_json::to_string_pretty(&value).unwrap())
        .expect("corrupt sealed");
    write_private_new(&backup, legacy.as_bytes()).expect("plant backup");
    assert!(!startup_cleanup(&paths).expect("sweep corrupt"));
    assert!(backup.exists(), "unverifiable backup survives");
    let _ = std::fs::remove_dir_all(&paths.root);
}
