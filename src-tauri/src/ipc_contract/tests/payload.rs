//! Frozen payload-shape checks. Split out of the IPC contract suite.

use super::*;

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
            completed_at: None,
            prompt: None,
        }),
        frozen("IdeaItem"),
    );
    assert_keys(
        "DayActivity",
        keys_of(&DayActivity {
            date: "2026-10-19".to_string(),
            ideas_completed: 2,
            intensity: 2,
        }),
        frozen("DayActivity"),
    );
    assert_keys(
        "WeeklyVelocity",
        keys_of(&WeeklyVelocity {
            this_week: 2,
            last_week: 1,
        }),
        frozen("WeeklyVelocity"),
    );
    assert_keys(
        "ProductivityMetrics",
        keys_of(&ProductivityMetrics {
            daily_activity: Vec::new(),
            current_streak: 3,
            weekly_velocity: WeeklyVelocity {
                this_week: 2,
                last_week: 1,
            },
            avg_cycle_hours: Some(26.5),
            project: None,
        }),
        frozen("ProductivityMetrics"),
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

