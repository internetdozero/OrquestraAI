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
    use crate::ideas::productivity::{DayActivity, ProductivityMetrics, WeeklyVelocity};
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
            has_scripts: false,
            script_files: Vec::new(),
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

    mod payload;

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
