//! Regression tests for the nested-envelope incident: a v1-only writer
//! sealed already-sealed files, stacking `v1(v2(...))` layers until the
//! reader — which opens a single layer — saw an empty catalog and the
//! operator's credentials "vanished". The seal path now refuses envelope
//! input, the reader unwraps and normalizes, and unknown versions fail
//! closed without touching disk.

use orquestra_core::paths::OrquestraPaths;

use super::backup::{migrate_backup_path, recover_plaintext_from_backup};
use super::envelope;
use super::io::{persist_sealed, write_private_new};
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
        "orquestra-nested-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("scratch");
    OrquestraPaths {
        root: root.clone(),
        vault: root.clone(),
        active: root,
    }
}

fn test_key(paths: &OrquestraPaths) -> [u8; 32] {
    key::file_key(paths).expect("test file key")
}

#[test]
fn seal_refuses_already_sealed_bytes() {
    let paths = test_paths("seal-guard");
    let legacy = r#"{"services":{"svc":{"keys":[]}}}"#;
    let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
    let error = seal_text(&paths, &sealed, AAD_VAULT).expect_err("nesting must fail");
    assert!(error.contains("already a sealed envelope"), "{error}");
    let key = test_key(&paths);
    let error = seal_with_key(&key, sealed.as_bytes(), AAD_VAULT).expect_err("nesting must fail");
    assert!(error.contains("already a sealed envelope"), "{error}");
    // Plain domain JSON still seals.
    seal_text(&paths, legacy, AAD_VAULT).expect("plain seals");
    let _ = std::fs::remove_dir_all(&paths.root);
}

#[test]
fn open_stored_unwraps_a_doubly_sealed_file_and_normalizes_it() {
    let paths = test_paths("double");
    let target = paths.root.join("vault.json");
    let catalog = r#"{"services":{"svc":{"keys":["k1"]}}}"#;
    let key = test_key(&paths);
    let inner = envelope::seal(&key, catalog.as_bytes(), AAD_VAULT).expect("inner");
    let outer = envelope::seal(&key, inner.as_bytes(), AAD_VAULT).expect("outer");
    persist_sealed(&target, &outer).expect("write nested");

    let (text, was_legacy) = open_stored(&paths, &target, &outer, AAD_VAULT).expect("nested opens");
    assert!(!was_legacy);
    assert_eq!(text, catalog);

    // The file was normalized back to a single layer on read.
    let stored = std::fs::read_to_string(&target).expect("re-read");
    let value: serde_json::Value = serde_json::from_str(&stored).expect("envelope");
    assert!(envelope::is_sealed(&value));
    let (reopened, _) = open_stored(&paths, &target, &stored, AAD_VAULT).expect("reopen");
    assert!(!envelope::is_envelope_bytes(reopened.as_bytes()));
    assert_eq!(reopened, catalog);
    let _ = std::fs::remove_dir_all(&paths.root);
}

#[test]
fn open_stored_unwraps_the_incident_shape_v1_over_v2_over_v2() {
    let paths = test_paths("triple");
    let target = paths.root.join("vault.json");
    let catalog = r#"{"services":{"svc":{"keys":["k1"]}}}"#;
    let key = test_key(&paths);
    let inner = envelope::seal(&key, catalog.as_bytes(), AAD_VAULT).expect("inner");
    let middle = envelope::seal(&key, inner.as_bytes(), AAD_VAULT).expect("middle");
    let outer = envelope::seal_v1_for_tests(&key, middle.as_bytes()).expect("v1 outer");
    persist_sealed(&target, &outer).expect("write nested");

    let (text, was_legacy) = open_stored(&paths, &target, &outer, AAD_VAULT).expect("nested opens");
    assert!(!was_legacy);
    assert_eq!(text, catalog);

    let stored = std::fs::read_to_string(&target).expect("re-read");
    let value: serde_json::Value = serde_json::from_str(&stored).expect("envelope");
    assert_eq!(envelope::sealed_version(&value), Some(2));
    let _ = std::fs::remove_dir_all(&paths.root);
}

#[test]
fn unknown_envelope_version_fails_closed_without_touching_disk() {
    let paths = test_paths("unknown-version");
    let target = paths.root.join("vault.json");
    let stored = r#"{"orquestraSealed":99,"alg":"AES-256-GCM","nonce":"x","data":"y"}"#;
    let error = open_stored(&paths, &target, stored, AAD_VAULT).expect_err("must fail");
    assert!(
        error.contains("unsupported sealed envelope version"),
        "{error}"
    );
    assert!(!target.exists(), "file must not be created");
    assert!(
        !migrate_backup_path(&target).exists(),
        "no backup may be taken"
    );
    let _ = std::fs::remove_dir_all(&paths.root);
}

#[test]
fn backup_recovery_unwraps_a_nested_backup() {
    let paths = test_paths("nested-backup");
    let target = paths.root.join("vault.json");
    let catalog = r#"{"services":{"svc":{"keys":["k1"]}}}"#;
    let key = test_key(&paths);
    let inner = envelope::seal(&key, catalog.as_bytes(), AAD_VAULT).expect("inner");
    let nested_backup = envelope::seal(&key, inner.as_bytes(), AAD_VAULT).expect("backup");
    write_private_new(&migrate_backup_path(&target), nested_backup.as_bytes())
        .expect("plant backup");
    assert_eq!(
        recover_plaintext_from_backup(&paths, &target, AAD_VAULT).as_deref(),
        Some(catalog)
    );

    // A backup that opens to nothing usable is refused instead of being
    // re-sealed into a deeper nesting.
    use aes_gcm::aead::KeyInit;
    let foreign: [u8; 32] = aes_gcm::Aes256Gcm::generate_key(&mut aes_gcm::aead::OsRng).into();
    let sealed_foreign = envelope::seal(&foreign, catalog.as_bytes(), AAD_VAULT).expect("foreign");
    write_private_new(&migrate_backup_path(&target), sealed_foreign.as_bytes())
        .expect("plant foreign backup");
    assert!(
        recover_plaintext_from_backup(&paths, &target, AAD_VAULT).is_none(),
        "unopenable backup must not heal"
    );
    let _ = std::fs::remove_dir_all(&paths.root);
}
