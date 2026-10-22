//! File-key tests, kept out of `mod.rs`: they exercise key resolution,
//! fallback stability and the keyring promotion path.

use orquestra_core::paths::OrquestraPaths;

use super::*;

fn test_paths(name: &str) -> OrquestraPaths {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "orquestra-at-rest-{name}-{}-{nanos}",
        std::process::id()
    ));
    OrquestraPaths {
        root: root.clone(),
        vault: root.clone(),
        active: root,
    }
}

#[test]
fn fallback_key_is_stable_per_root_and_unique_across_roots() {
    let first = test_paths("stable-a");
    let second = test_paths("stable-b");
    let key_a1 = file_key(&first).expect("key");
    let key_a2 = file_key(&first).expect("key again");
    assert_eq!(key_a1, key_a2, "cached key must be stable");
    let key_b = file_key(&second).expect("other root");
    assert_ne!(key_a1, key_b, "distinct roots get distinct keys");
    for paths in [&first, &second] {
        let key_path = fallback_path(&paths.root);
        assert!(key_path.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = std::fs::remove_dir_all(&paths.root);
    }
}

#[test]
fn fallback_key_promotes_into_an_empty_keyring() {
    // SSH → desktop handoff: the fallback bytes move into the keyring
    // with no new randomness; an existing keyring key is never
    // overwritten by a different fallback.
    let _serial = test_keyring_lock();
    test_keyring_clear();
    let fallback = [0x42u8; 32];
    let promoted = promote_fallback_to_keyring(&fallback).expect("promote");
    assert_eq!(promoted, fallback);
    assert_eq!(read_keyring_key().expect("read").as_ref(), Some(&fallback));
    let other = [0x99u8; 32];
    let kept = promote_fallback_to_keyring(&other).expect("keep");
    assert_eq!(kept, fallback, "the existing keyring key wins");
    test_keyring_clear();
}

#[test]
fn read_with_sealed_file_and_no_key_creates_nothing() {
    // A read against a stranded sealed file must fail closed, not mint
    // a fresh key that could never open it (the #323 lockout).
    let paths = test_paths("no-create-on-read");
    std::fs::create_dir_all(&paths.root).expect("scratch");
    let key = file_key(&paths).expect("first-write key");
    let sealed =
        super::super::envelope::seal(&key, b"{}", super::super::envelope::AAD_VAULT).expect("seal");
    std::fs::write(paths.root.join("vault.json"), &sealed).expect("plant sealed");
    // Drop the only key: reads must now fail without recreating it.
    std::fs::remove_file(fallback_path(&paths.root)).expect("drop key");
    cache()
        .lock()
        .expect("cache")
        .remove(&paths.root.to_string_lossy().into_owned());
    assert!(
        !fallback_exists(&paths.root),
        "precondition: no fallback key on disk"
    );
    assert!(
        read_fallback_key(&paths.root).expect("readable").is_none(),
        "no fallback key on disk"
    );
    assert!(
        file_key(&paths).is_err(),
        "even the write path must not mint a key over stranded sealed files"
    );
    assert!(
        !fallback_exists(&paths.root),
        "a failed read must not create a key file"
    );
    let _ = std::fs::remove_dir_all(&paths.root);
}
