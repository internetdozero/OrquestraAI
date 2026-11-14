//! Integration tests for granular MCP CLI subcommands (`ownhub mcp <cmd>`).

mod common;

use ownhub_lib::{dispatch_native_cli, CliError};

fn run_mcp(args: &[&str]) -> Result<String, CliError> {
    let full_args: Vec<String> = std::iter::once("mcp".to_string())
        .chain(args.iter().map(|s| s.to_string()))
        .collect();
    dispatch_native_cli(&full_args).expect("mcp root must be dispatched")
}

#[test]
fn mcp_list_and_status_isolate_cleanly() {
    common::with_temp_home("cli-mcp-list", |_| {
        let list_out = run_mcp(&["list", "--json"]).expect("list json");
        let val: serde_json::Value = serde_json::from_str(&list_out).expect("valid JSON array");
        assert!(val.is_array());

        let status_out = run_mcp(&["status", "--json"]).expect("status json");
        let status_val: serde_json::Value =
            serde_json::from_str(&status_out).expect("valid JSON obj");
        assert!(status_val.get("totalServers").is_some());
    });
}

#[test]
fn mcp_add_and_remove_lifecycle() {
    common::with_temp_home("cli-mcp-lifecycle", |_| {
        let add_out = run_mcp(&[
            "add",
            "--id",
            "test-srv",
            "--name",
            "Test Server",
            "--command",
            "echo",
            "--arg",
            "hello",
            "--env",
            "TEST_ENV=123",
            "--json",
        ])
        .expect("add server");
        let added_val: serde_json::Value = serde_json::from_str(&add_out).expect("valid JSON obj");
        assert_eq!(added_val["id"], "test-srv");

        let inspect_out = run_mcp(&["inspect", "test-srv", "--json"]).expect("inspect server");
        let ins_val: serde_json::Value =
            serde_json::from_str(&inspect_out).expect("valid JSON obj");
        assert_eq!(ins_val["id"], "test-srv");

        let rm_out = run_mcp(&["remove", "test-srv", "--json"]).expect("remove server");
        let rm_val: serde_json::Value = serde_json::from_str(&rm_out).expect("valid JSON obj");
        assert_eq!(rm_val["deleted"], true);
    });
}

#[test]
fn mcp_usage_errors() {
    common::with_temp_home("cli-mcp-usage", |_| {
        assert!(matches!(run_mcp(&["inspect"]), Err(CliError::Usage(_))));
        assert!(matches!(
            run_mcp(&["unknown-subcommand"]),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            run_mcp(&["add", "--id", "foo"]),
            Err(CliError::Usage(_))
        ));
    });
}
