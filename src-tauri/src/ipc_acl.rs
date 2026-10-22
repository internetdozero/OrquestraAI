//! Test-only ACL guard for the Tauri capability split.
//!
//! `build.rs` declares every invoke-handler command deny-by-default via
//! `AppManifest::commands`, so a command the scope sets forget is denied at
//! runtime. The test below pins the rest: the frozen `capabilityScopes` in
//! `tests/fixtures/ipc-contract.json` must partition the registered commands
//! exactly, the `permissions/*.toml` sets must match the frozen mapping
//! (through `allow-<command>`), the destructive commands must stay out of
//! `safe-read`, and the withheld `vault-blind-ops` scope must be granted to
//! no window — otherwise `vault_exec`/`vault_http_request` would be callable
//! from the webview with no UI consumer to justify them.
//!
//! Test-only: nothing in this module is compiled into the binary.

/// Scopes that must be granted to NO window: blind key use with no confirmed
/// UI caller. Re-enabling one means adding a capability file AND a gated
/// frontend wrapper; until then the acl test below fails if any capability
/// references them.
pub(crate) const WITHHELD_SCOPES: &[&str] = &["vault-blind-ops"];

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::PathBuf};

    use serde_json::Value;

    use super::WITHHELD_SCOPES;

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn read(relative: &str) -> String {
        let path = manifest_dir().join(relative);
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
    }

    /// Command names inside `tauri::generate_handler![ .. ]` in `lib.rs`.
    fn registered_commands() -> BTreeSet<String> {
        let source = read("src/lib.rs");
        let opener = "tauri::generate_handler![";
        let start = source
            .find(opener)
            .expect("lib.rs should register an invoke_handler")
            + opener.len();
        // The list holds bare identifiers, so the first `]` closes it.
        let body = &source[start..];
        let end = body.find(']').expect("invoke_handler list should close");
        let name = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';

        body[..end]
            .split(|ch: char| !name(ch))
            .filter(|token| !token.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn frozen_scopes() -> serde_json::Map<String, Value> {
        let raw = read("tests/fixtures/ipc-contract.json");
        let fixture: Value =
            serde_json::from_str(&raw).expect("ipc-contract.json is not valid JSON");
        fixture["capabilityScopes"]
            .as_object()
            .expect("fixture has no capabilityScopes object")
            .clone()
    }

    #[test]
    fn capability_scopes_partition_all_commands() {
        let registered = registered_commands();
        let fixture_scopes = frozen_scopes();
        let mut frozen: BTreeSet<String> = BTreeSet::new();
        for (scope, commands) in &fixture_scopes {
            let list = commands
                .as_array()
                .unwrap_or_else(|| panic!("capabilityScopes.{scope} must be an array"));
            for command in list {
                let name = command
                    .as_str()
                    .unwrap_or_else(|| panic!("capabilityScopes.{scope} must hold strings"));
                assert!(
                    frozen.insert(name.to_string()),
                    "{name:?} is in more than one frozen scope"
                );
            }
        }
        let missing: Vec<_> = registered.difference(&frozen).cloned().collect();
        let extra: Vec<_> = frozen.difference(&registered).cloned().collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "frozen capabilityScopes do not partition the registered commands.\n  unscoped (denied at runtime): {missing:?}\n  not a registered command: {extra:?}\n  Update the fixture together with src-tauri/permissions/*.toml."
        );

        // The TOML sets must grant exactly the frozen mapping.
        let dir = manifest_dir().join("permissions");
        let mut granted: BTreeSet<(String, String)> = BTreeSet::new();
        let mut set_ids: BTreeSet<String> = BTreeSet::new();
        let entries = fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()));
        for entry in entries {
            let path = entry.expect("permission entry should read").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            for (set, allows) in scope_sets_in(&text) {
                assert!(
                    set_ids.insert(set.clone()),
                    "duplicate permission set {set:?}"
                );
                for allow in allows {
                    granted.insert((set.clone(), allow));
                }
            }
        }
        let expected: BTreeSet<(String, String)> = fixture_scopes
            .iter()
            .flat_map(|(scope, commands)| {
                commands
                    .as_array()
                    .expect("scope must be an array")
                    .iter()
                    .map(move |command| {
                        (
                            scope.clone(),
                            command
                                .as_str()
                                .expect("scope entry must be a string")
                                .to_string(),
                        )
                    })
            })
            .collect();
        assert_eq!(
            granted, expected,
            "src-tauri/permissions/*.toml drifted from the frozen capabilityScopes"
        );

        // Destructive commands must never sit in the default-granted set.
        let safe = fixture_scopes["safe-read"]
            .as_array()
            .expect("fixture has no safe-read scope");
        for destructive in [
            "vault_exec",
            "vault_http_request",
            "vault_save_service",
            "vault_delete_service",
            "vault_set_primary",
            "vault_remove_key",
            "vault_test_connection",
            "reveal_mcp_secret",
            "apply_app_update",
            "restart_app",
            "sync_official_extensions",
            "purge_sessions",
            "save_retention_policy",
            "delete_skill",
            "delete_mcp",
            "delete_preset",
        ] {
            assert!(
                !safe.iter().any(|entry| entry.as_str() == Some(destructive)),
                "{destructive:?} must not be in the safe-read scope"
            );
        }

        // Capability files may only reference core permissions and the defined
        // sets. Every scope but the withheld ones must reach the main window;
        // withheld scopes must be granted nowhere (deny-by-default keeps them
        // denied at runtime even though their commands stay registered).
        let cap_dir = manifest_dir().join("capabilities");
        let mut main_grants: BTreeSet<String> = BTreeSet::new();
        let mut all_grants: BTreeSet<String> = BTreeSet::new();
        let caps = fs::read_dir(&cap_dir)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", cap_dir.display()));
        for entry in caps {
            let path = entry.expect("capability entry should read").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let capability: Value =
                serde_json::from_str(&raw).expect("capability file must be valid JSON");
            let permissions = capability["permissions"]
                .as_array()
                .unwrap_or_else(|| panic!("{} has no permissions array", path.display()));
            let on_main = capability["windows"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .any(|window| window.as_str() == Some("main"));
            for permission in permissions {
                let name = permission.as_str().unwrap_or_else(|| {
                    panic!(
                        "{} holds a scoped permission object; extend the test to cover scopes",
                        path.display()
                    )
                });
                if name.starts_with("core:") {
                    continue;
                }
                assert!(
                    set_ids.contains(name),
                    "{} references unknown permission set {name:?}",
                    path.display()
                );
                all_grants.insert(name.to_string());
                if on_main {
                    main_grants.insert(name.to_string());
                }
            }
        }
        for scope in fixture_scopes.keys() {
            if WITHHELD_SCOPES.contains(&scope.as_str()) {
                assert!(
                    !all_grants.contains(scope),
                    "withheld scope {scope:?} must be granted to no window; add a gated caller before re-enabling"
                );
            } else {
                assert!(
                    main_grants.contains(scope),
                    "no capability grants the {scope:?} set to the main window"
                );
            }
        }
    }

    /// `(set identifier, allow-<command> tokens)` from one permissions file.
    /// Only `[[set]]` sections with a `permissions = [...]` array are read;
    /// comments and descriptions are ignored.
    fn scope_sets_in(text: &str) -> Vec<(String, Vec<String>)> {
        let mut sets = Vec::new();
        let mut current: Option<(String, Vec<String>)> = None;
        let mut in_list = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("[[set]]") {
                if let Some(done) = current.take() {
                    sets.push(done);
                }
                current = Some((String::new(), Vec::new()));
                in_list = false;
            } else if let Some((set, allows)) = current.as_mut() {
                if line.starts_with("identifier") {
                    *set = line
                        .split('"')
                        .nth(1)
                        .unwrap_or_else(|| panic!("set identifier must be quoted"))
                        .to_string();
                } else if line.starts_with("permissions") {
                    in_list = true;
                } else if in_list {
                    for token in line.split('"').skip(1).step_by(2) {
                        if let Some(command) = token.strip_prefix("allow-") {
                            allows.push(command.replace('-', "_"));
                        }
                    }
                    if line.contains(']') {
                        in_list = false;
                    }
                }
            }
        }
        if let Some(done) = current {
            sets.push(done);
        }
        sets
    }
}
