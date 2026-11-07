use super::*;

#[test]
fn sessions_cli_usage_errors() {
    assert!(matches!(
        run(&["unknown".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(run(&["pin".to_string()]), Err(CliError::Usage(_))));
    assert!(matches!(
        run(&["unpin".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["delete".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["export".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["purge".to_string(), "--yes".to_string()]),
        Err(CliError::Usage(_))
    ));
}

#[test]
fn sessions_cli_list_runs_cleanly() {
    let _home = crate::test_env::isolated_home();
    let args = ["list".to_string(), "--json".to_string()];
    let res = run(&args);
    assert!(res.is_ok(), "sessions list --json failed: {:?}", res.err());
    let output = res.unwrap();
    let val: serde_json::Value = serde_json::from_str(&output).expect("valid JSON array");
    assert!(val.is_array());
}

#[test]
fn sessions_cli_purge_dry_run() {
    let args = [
        "purge".to_string(),
        "--older-than-days".to_string(),
        "9999".to_string(),
        "--json".to_string(),
    ];
    let res = run(&args);
    assert!(res.is_ok(), "purge dry run failed: {:?}", res.err());
    let output = res.unwrap();
    let val: serde_json::Value = serde_json::from_str(&output).expect("valid JSON obj");
    assert_eq!(val["dryRun"], true);
}

#[test]
fn sessions_cli_policy_show_and_set() {
    let show_args = [
        "policy".to_string(),
        "show".to_string(),
        "--json".to_string(),
    ];
    let show_res = run(&show_args).expect("policy show");
    let show_val: serde_json::Value = serde_json::from_str(&show_res).expect("valid JSON");
    assert!(show_val.get("defaultTtlDays").is_some());

    let set_args = [
        "policy".to_string(),
        "set".to_string(),
        "--default-ttl".to_string(),
        "20".to_string(),
        "--auto-purge".to_string(),
        "off".to_string(),
        "--override".to_string(),
        "codex=10".to_string(),
        "--json".to_string(),
    ];
    let set_res = run(&set_args).expect("policy set");
    let set_val: serde_json::Value = serde_json::from_str(&set_res).expect("valid JSON");
    assert_eq!(set_val["defaultTtlDays"], 20);
    assert_eq!(set_val["autoPurgeEnabled"], false);
    assert_eq!(set_val["harnessOverrides"]["codex"], 10);
}
