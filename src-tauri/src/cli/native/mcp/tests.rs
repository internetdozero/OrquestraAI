use super::*;

#[test]
fn mcp_cli_rejects_unknown_subcommand() {
    let args = ["invalid-cmd".to_string()];
    assert!(matches!(run(&args), Err(CliError::Usage(_))));
}

#[test]
fn mcp_cli_rejects_inspect_without_id() {
    let args = ["inspect".to_string()];
    assert!(matches!(run(&args), Err(CliError::Usage(_))));
}

#[test]
fn mcp_cli_rejects_set_state_with_invalid_state() {
    let args = [
        "set-state".to_string(),
        "test-server".to_string(),
        "bad".to_string(),
    ];
    assert!(matches!(run(&args), Err(CliError::Usage(_))));
}

#[test]
fn mcp_cli_list_runs_cleanly() {
    let _home = crate::test_env::isolated_home();
    let args = ["list".to_string(), "--json".to_string()];
    let res = run(&args);
    assert!(res.is_ok(), "mcp list --json failed: {:?}", res.err());
    let output = res.unwrap();
    let val: serde_json::Value = serde_json::from_str(&output).expect("valid JSON array");
    assert!(val.is_array());
}

#[test]
fn mcp_cli_status_runs_cleanly() {
    let _home = crate::test_env::isolated_home();
    let args = ["status".to_string(), "--json".to_string()];
    let res = run(&args);
    assert!(res.is_ok(), "mcp status --json failed: {:?}", res.err());
    let output = res.unwrap();
    let val: serde_json::Value = serde_json::from_str(&output).expect("valid JSON object");
    assert!(val.get("totalServers").is_some());
    assert!(val.get("syncStatus").is_some());
}

#[test]
fn mcp_cli_targets_runs_cleanly() {
    let _home = crate::test_env::isolated_home();
    let args = ["targets".to_string(), "--json".to_string()];
    let res = run(&args);
    assert!(res.is_ok(), "mcp targets --json failed: {:?}", res.err());
    let output = res.unwrap();
    let val: serde_json::Value = serde_json::from_str(&output).expect("valid JSON array");
    assert!(val.is_array());
}
