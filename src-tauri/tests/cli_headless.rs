//! Integration coverage for the headless terminal flags (`--bridge-status`,
//! `--mcp-status`, `--mcp-deploy`, `--mcp-set`, `--preset`): the same
//! functions `main.rs` invokes, against a scratch `HOME` so the real
//! `~/.orquestra` is never touched.

mod common;

use orquestra_ai_lib::{apply_preset_json, bridge_status_json, deploy_mcp_json, mcp_status_json};
use orquestra_ai_lib::{deploy_bridge_json, set_mcp_state_cli};

fn parse(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("command output must be JSON")
}

#[test]
fn bridge_status_reports_json_on_empty_home() {
    common::with_temp_home("cli-bridge", |_| {
        let report = parse(&bridge_status_json().expect("bridge status"));
        assert!(report.is_array(), "status is one row per harness");
    });
}

#[test]
fn bridge_deploy_is_a_noop_report_without_harnesses() {
    common::with_temp_home("cli-deploy", |_| {
        // Nothing installed under the scratch HOME: the deploy walks zero
        // harnesses and still returns a well-formed report, never an error.
        let report = parse(&deploy_bridge_json().expect("bridge deploy"));
        assert!(report.is_object());
    });
}

#[test]
fn mcp_lifecycle_round_trips_through_the_cli_layer() {
    use std::collections::HashMap;

    common::with_temp_home("cli-mcp", |_| {
        let catalog = parse(&mcp_status_json().expect("mcp status"));
        assert!(catalog.is_array());

        // Unknown ids fail closed instead of creating phantom servers.
        assert!(set_mcp_state_cli("no-such-server", "global", true).is_err());

        orquestra_ai_lib::mcp::save_mcp(orquestra_ai_lib::mcp::McpServer {
            id: "it-server".to_string(),
            name: "Integration Test Server".to_string(),
            description: String::new(),
            command: "echo".to_string(),
            args: Vec::new(),
            env: HashMap::new(),
            url: None,
            headers: HashMap::new(),
            global_enabled: false,
            enabled_harnesses: Vec::new(),
            official: false,
        })
        .expect("save server");

        let enabled = parse(&set_mcp_state_cli("it-server", "global", true).expect("enable"));
        assert_eq!(enabled["id"], "it-server");
        assert_eq!(enabled["enabled"], true);
        let listed = parse(&mcp_status_json().expect("mcp status"));
        let server = listed
            .as_array()
            .expect("array")
            .iter()
            .find(|entry| entry["id"] == "it-server")
            .expect("saved server is listed");
        assert_eq!(server["globalEnabled"], true);

        // `--mcp-deploy` compiles the catalog without touching real configs.
        let deployed = parse(&deploy_mcp_json().expect("mcp deploy"));
        assert!(deployed.get("changed").is_some());
        assert!(deployed.get("targets").is_some());
        assert!(deployed.get("servers").is_some());

        parse(&set_mcp_state_cli("it-server", "global", false).expect("disable"));
        let listed = parse(&mcp_status_json().expect("mcp status"));
        let server = listed
            .as_array()
            .expect("array")
            .iter()
            .find(|entry| entry["id"] == "it-server")
            .expect("saved server is listed");
        assert_eq!(server["globalEnabled"], false);
    });
}

#[test]
fn preset_flag_applies_builtins_and_rejects_unknown() {
    common::with_temp_home("cli-preset", |_| {
        assert!(apply_preset_json("no-such-preset").is_err());
        // `full-power` takes the whole (empty) vault: no warnings, no writes
        // outside the scratch HOME.
        let report = parse(&apply_preset_json("full-power").expect("preset"));
        assert!(report.is_object());
    });
}
