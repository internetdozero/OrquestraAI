#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Frozen wire contract between the Rust core and the React client.
//!
//! The frontend talks to the core through `invoke("<command>")` and reads the
//! camelCase JSON these structs serialize to, but nothing in the compiler links
//! the two sides. These tests read the same files the client does — the
//! `invoke_handler!` list, the CLI `match` arms and a snapshot of every payload
//! shape in `src-tauri/tests/fixtures/ipc-contract.json` — so a refactor that
//! renames a command, drops a flag or changes field casing fails here instead of in the shipped app.
//!
//! Test-only: nothing in this module is compiled into the binary.

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::PathBuf};

    use serde::Serialize;
    use serde_json::Value;

    use crate::bridge::{BridgeReport, HarnessStatus, SecondaryAction, SecondaryStatus};
    use crate::ideas::types::{BlackboardNote, IdeaItem};
    use crate::mcp::{McpServer, McpTargetStatus};
    use crate::presets::{Preset, PresetReport};
    use crate::skills::{HarnessSummary, SkillItem};

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn read(relative: &str) -> String {
        let path = manifest_dir().join(relative);
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
    }

    fn fixture() -> Value {
        let raw = read("tests/fixtures/ipc-contract.json");
        serde_json::from_str(&raw).expect("tests/fixtures/ipc-contract.json is not valid JSON")
    }

    fn frozen_list(key: &str) -> BTreeSet<String> {
        fixture()[key]
            .as_array()
            .unwrap_or_else(|| panic!("fixture has no `{key}` array"))
            .iter()
            .map(|item| {
                item.as_str()
                    .unwrap_or_else(|| panic!("`{key}` must hold strings"))
                    .to_string()
            })
            .collect()
    }

    /// Serialized key set of one value, as the client sees it.
    fn keys_of<T: Serialize>(value: &T) -> BTreeSet<String> {
        match serde_json::to_value(value).expect("value should serialize") {
            Value::Object(map) => map.keys().cloned().collect(),
            other => panic!("expected an object payload, got {other}"),
        }
    }

    fn assert_keys(label: &str, actual: BTreeSet<String>, expected: BTreeSet<String>) {
        assert_eq!(
            actual, expected,
            "payload shape of `{label}` drifted from the frozen contract"
        );
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

    /// Every `invoke("<command>")` in the TypeScript client.
    fn invoked_commands(source: &str) -> BTreeSet<String> {
        let chars: Vec<char> = source.chars().collect();
        let mut found = BTreeSet::new();
        let mut index = 0;

        while index < chars.len() {
            if chars[index..].starts_with(&['i', 'n', 'v', 'o', 'k', 'e']) {
                let mut cursor = index + "invoke".len();

                // Skip the generic parameter, `invoke<McpServer[]>(`.
                if chars.get(cursor) == Some(&'<') {
                    while cursor < chars.len() && chars[cursor] != '>' {
                        cursor += 1;
                    }
                    cursor += 1;
                }

                if chars.get(cursor) == Some(&'(') {
                    cursor += 1;
                    while chars.get(cursor).is_some_and(|c| c.is_whitespace()) {
                        cursor += 1;
                    }

                    if chars.get(cursor) == Some(&'"') {
                        cursor += 1;
                        let start = cursor;
                        while cursor < chars.len() && chars[cursor] != '"' {
                            cursor += 1;
                        }

                        let name: String = chars[start..cursor].iter().collect();
                        let is_command = !name.is_empty()
                            && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                            && name
                                .chars()
                                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');

                        if is_command {
                            found.insert(name);
                        }
                    }
                }

                index = cursor.max(index + 1);
            } else {
                index += 1;
            }
        }

        found
    }

    fn skill_item() -> SkillItem {
        SkillItem {
            id: "caveman".to_string(),
            name: "Caveman".to_string(),
            description: "Ultra-compressed output mode.".to_string(),
            category: "AI & Agents".to_string(),
            tokens: 840,
            global_enabled: true,
            harness_enabled: true,
            inherited_from_global: false,
            official: false,
        }
    }

    fn harness_status() -> HarnessStatus {
        HarnessStatus {
            id: "codex".to_string(),
            label: "Codex".to_string(),
            kind: "skills".to_string(),
            target: "~/.codex/skills".to_string(),
            skills_paths: vec!["~/.codex/skills".to_string()],
            skills_linked: true,
            instructions_path: Some("~/.codex/AGENTS.md".to_string()),
            instructions_linked: true,
            config_path: None,
            config_linked: false,
            secondary_paths: vec![SecondaryStatus {
                path: ".agents/skills".to_string(),
                action: SecondaryAction::Quarantine,
                cleared: true,
                rationale: "Shared user root".to_string(),
            }],
            secondary_cleared: true,
            installed: true,
            supported: true,
            bridged: true,
            note: None,
        }
    }

    #[test]
    fn registered_commands_match_the_frozen_contract() {
        let actual = registered_commands();
        let expected = frozen_list("commands");

        let missing: Vec<_> = expected.difference(&actual).cloned().collect();
        let extra: Vec<_> = actual.difference(&expected).cloned().collect();

        assert!(
            missing.is_empty() && extra.is_empty(),
            "IPC command surface changed.\n  no longer registered: {missing:?}\n  not in the fixture: {extra:?}\n\
             Update src-tauri/tests/fixtures/ipc-contract.json and the matching src/lib/*.ts caller together."
        );
    }

    #[test]
    fn every_frontend_invocation_is_registered() {
        let mut invoked = BTreeSet::new();
        for file in [
            "src/lib/tauri.ts",
            "src/lib/mcps.ts",
            "src/lib/bridge.ts",
            "src/lib/presets.ts",
            "src/lib/sessions.ts",
            "src/lib/janitor.ts",
            "src/lib/ideas.ts",
        ] {
            invoked.extend(invoked_commands(&read(&format!("../{file}"))));
        }

        assert!(
            !invoked.is_empty(),
            "no invoke() calls were detected; the scanner needs updating"
        );

        let registered = registered_commands();
        let unknown: Vec<_> = invoked.difference(&registered).cloned().collect();
        assert!(
            unknown.is_empty(),
            "the client calls commands the core does not register: {unknown:?}"
        );

        let unused: Vec<_> = registered.difference(&invoked).cloned().collect();
        assert!(
            unused.is_empty(),
            "the core registers commands no client calls: {unused:?}"
        );
    }

    #[test]
    fn payload_shapes_match_the_frozen_contract() {
        let shapes = &fixture()["payloadShapes"];

        let frozen = |name: &str| -> BTreeSet<String> {
            shapes[name]
                .as_array()
                .unwrap_or_else(|| panic!("fixture has no payloadShapes.{name}"))
                .iter()
                .map(|item| item.as_str().expect("shape keys must be strings").to_string())
                .collect()
        };

        assert_keys("SkillItem", keys_of(&skill_item()), frozen("SkillItem"));
        assert_keys(
            "HarnessSummary",
            keys_of(&HarnessSummary {
                id: "global".to_string(),
                active: 2,
                total: 7,
            }),
            frozen("HarnessSummary"),
        );
        assert_keys(
            "McpServer",
            keys_of(&McpServer {
                id: "github".to_string(),
                name: "github".to_string(),
                description: "Repositories and issues.".to_string(),
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()],
                env: [("GITHUB_TOKEN".to_string(), "ghp_test".to_string())]
                    .into_iter()
                    .collect(),
                url: None,
                headers: Default::default(),
                global_enabled: false,
                enabled_harnesses: vec!["codex".to_string()],
                official: false,
            }),
            frozen("McpServer"),
        );
        assert_keys(
            "McpTargetStatus",
            keys_of(&McpTargetStatus {
                id: "codex".to_string(),
                label: "Codex".to_string(),
                config_path: "~/.codex/config.toml".to_string(),
                installed: true,
            }),
            frozen("McpTargetStatus"),
        );
        assert_keys(
            "Preset",
            keys_of(&Preset {
                id: "stealth".to_string(),
                name: "Stealth".to_string(),
                description: "Zero-bloat profile.".to_string(),
                icon: "shield".to_string(),
                skills: vec!["rtk".to_string()],
                mcps: vec!["headroom".to_string()],
                harness_skills: [("antigravity".to_string(), vec!["rtk".to_string()])]
                    .into_iter()
                    .collect(),
                harness_mcps: Default::default(),
                all_skills: false,
                all_mcps: false,
                builtin: true,
                active: false,
            }),
            frozen("Preset"),
        );
        assert_keys(
            "PresetReport",
            keys_of(&PresetReport {
                id: "stealth".to_string(),
                name: "Stealth".to_string(),
                skills_enabled: 1,
                skills_disabled: 427,
                mcps_enabled: 2,
                mcps_disabled: 3,
                configs_changed: vec!["~/.codex/config.toml".to_string()],
                warnings: vec!["skill “modular-start” is not in the vault".to_string()],
            }),
            frozen("PresetReport"),
        );
        assert_keys(
            "SecondaryStatus",
            keys_of(&harness_status().secondary_paths[0]),
            frozen("SecondaryStatus"),
        );
        assert_keys("HarnessStatus", keys_of(&harness_status()), frozen("HarnessStatus"));
        assert_keys(
            "IdeaItem",
            keys_of(&IdeaItem {
                id: 1,
                project: "vault".to_string(),
                title: "Native module".to_string(),
                description: None,
                category: "feature".to_string(),
                priority: "medium".to_string(),
                tags: None,
                status: "open".to_string(),
                created_at: "2026-10-18 12:00:00".to_string(),
                updated_at: "2026-10-18 12:00:00".to_string(),
            }),
            frozen("IdeaItem"),
        );
        assert_keys(
            "BlackboardNote",
            keys_of(&BlackboardNote {
                id: 1,
                project: "vault".to_string(),
                topic: "handoff".to_string(),
                sender: "builder".to_string(),
                target: "all".to_string(),
                note: "Phase 1 is green.".to_string(),
                is_read: false,
                created_at: "2026-10-18 12:00:00".to_string(),
            }),
            frozen("BlackboardNote"),
        );
        assert_keys(
            "BridgeReport",
            keys_of(&BridgeReport {
                backup_dir: Some("~/.orquestra/backups/pre-fusion-1".to_string()),
                bridged: 6,
                total: 9,
                harnesses: vec![harness_status()],
                adopted: Vec::new(),
                quarantined: Vec::new(),
                instructions_imported: Vec::new(),
                warnings: Vec::new(),
                errors: Vec::new(),
            }),
            frozen("BridgeReport"),
        );
    }

    #[test]
    fn secondary_actions_serialize_as_the_client_expects() {
        let mut serialized = BTreeSet::new();

        for (action, expected) in [
            (SecondaryAction::Project, "project"),
            (SecondaryAction::Quarantine, "quarantine"),
            (SecondaryAction::AdoptOnly, "adoptOnly"),
        ] {
            assert_eq!(
                serde_json::to_value(action).expect("action should serialize"),
                Value::String(expected.to_string()),
                "the client's SecondaryAction union no longer matches the wire format"
            );
            serialized.insert(expected.to_string());
        }

        let frozen: BTreeSet<String> = fixture()["payloadShapes"]["SecondaryAction"]
            .as_array()
            .expect("fixture has no payloadShapes.SecondaryAction")
            .iter()
            .map(|item| item.as_str().expect("variants must be strings").to_string())
            .collect();

        assert_eq!(
            serialized, frozen,
            "the SecondaryAction variants on the wire drifted from the contract"
        );
    }

    /// `-h`, `--mcp-set`: one or two dashes, then a letter and flag characters.
    fn is_flag_literal(literal: &str) -> bool {
        let dashes = literal.chars().take_while(|c| *c == '-').count();
        if dashes != 1 && dashes != 2 {
            return false;
        }

        let rest = &literal[dashes..];
        rest.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    }

    /// Flag literals the binary matches on: `Some("--mcp-set" | "-h")`.
    fn handled_flags(source: &str) -> BTreeSet<String> {
        source
            .split('"')
            .skip(1)
            .step_by(2)
            .filter(|literal| is_flag_literal(literal))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn cli_flags_are_still_handled_by_the_binary() {
        let actual = handled_flags(&read("src/main.rs"));
        let expected = frozen_list("cliFlags");

        let missing: Vec<_> = expected.difference(&actual).cloned().collect();
        let extra: Vec<_> = actual.difference(&expected).cloned().collect();

        assert!(
            missing.is_empty() && extra.is_empty(),
            "headless CLI surface changed.\n  no longer handled: {missing:?}\n  not in the fixture: {extra:?}\n\
             Update src-tauri/tests/fixtures/ipc-contract.json and any script calling these flags together."
        );
    }
}
