//! Kill-switch tests: the `enabled` default, toggle round-trips and the
//! blind refusal that keeps disabled services away from agents.

use super::super::store::VaultService;
use super::{execute, save_service, set_service_enabled, SaveServiceInput};

fn temp_paths(label: &str) -> orquestra_core::paths::OrquestraPaths {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "orquestra-vault-enabled-{label}-{}-{nanos}",
        std::process::id()
    ));
    orquestra_core::paths::OrquestraPaths {
        root: root.clone(),
        vault: root.clone(),
        active: root,
    }
}

fn save_input(id: &str) -> SaveServiceInput {
    SaveServiceInput {
        id: id.to_string(),
        label: "Svc".to_string(),
        preset: "custom".to_string(),
        description: None,
        keys: vec!["sk-secret-1".to_string()],
        custom_header: None,
        custom_prefix: None,
        custom_env: None,
        meta: None,
        key_tags: None,
    }
}

#[test]
fn legacy_services_without_a_flag_load_as_enabled() {
    let parsed: VaultService =
        serde_json::from_str(r#"{"id":"a","label":"A","preset":"custom","keys":[]}"#)
            .expect("legacy file must still parse");
    assert!(
        parsed.enabled,
        "pre-toggle vault files must load as enabled, never as a surprise lockout"
    );
}

#[test]
fn toggle_round_trip_preserves_the_flag_across_saves() {
    let paths = temp_paths("toggle");
    let first = save_service(&paths, save_input("svc")).expect("save");
    assert!(first.enabled, "new services start enabled");

    let off = set_service_enabled(&paths, "svc", false).expect("disable");
    assert!(!off.enabled);

    // The editor round-trips masks, not the kill-switch: a re-save must not
    // resurrect a disabled service.
    let resaved = save_service(
        &paths,
        SaveServiceInput {
            keys: vec![off.masked_keys[0].clone()],
            ..save_input("svc")
        },
    )
    .expect("resave");
    assert!(!resaved.enabled, "re-save dropped the disabled flag");

    let on = set_service_enabled(&paths, "svc", true).expect("enable");
    assert!(on.enabled);
    let _ = std::fs::remove_dir_all(&paths.root);
}

#[test]
fn unknown_service_toggle_fails_closed() {
    let paths = temp_paths("toggle-unknown");
    let err = set_service_enabled(&paths, "nope", false).expect_err("must fail");
    assert!(err.contains("unknown vault service"), "unexpected: {err}");
    let _ = std::fs::remove_dir_all(&paths.root);
}

#[test]
fn disabled_services_are_refused_before_any_spawn() {
    let paths = temp_paths("refuse");
    save_service(&paths, save_input("svc")).expect("save");
    set_service_enabled(&paths, "svc", false).expect("disable");
    let err = execute(&paths, "svc", &serde_json::json!(["printf", "hi"]))
        .expect_err("disabled service must be refused");
    assert!(err.contains("is disabled"), "unexpected: {err}");
    let _ = std::fs::remove_dir_all(&paths.root);
}
