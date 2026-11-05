use super::*;

#[test]
fn unknown_subcommands_show_usage() {
    assert!(matches!(run(&[]), Err(CliError::Usage(_))));
    assert!(matches!(run(&["bogus".to_string()]), Err(CliError::Usage(_))));
}

#[test]
fn key_number_is_one_based_and_positive() {
    assert_eq!(parse_key_number("5").unwrap(), 5);
    assert!(matches!(parse_key_number("0"), Err(CliError::Usage(_))));
    assert!(matches!(parse_key_number("abc"), Err(CliError::Usage(_))));
}

#[test]
fn set_enabled_rejects_bad_shapes_before_touching_disk() {
    for args in [
        vec![],
        vec!["svc".to_string()],
        vec!["svc".to_string(), "maybe".to_string()],
        vec![
            "svc".to_string(),
            "on".to_string(),
            "extra".to_string(),
            "junk".to_string(),
        ],
    ] {
        assert!(matches!(mutate::set_enabled(&args), Err(CliError::Usage(_))));
    }
}

#[test]
fn exec_rejects_empty_argv_before_touching_disk() {
    assert!(matches!(proxy::exec(&[]), Err(CliError::Usage(_))));
    assert!(matches!(proxy::exec(&["svc".to_string()]), Err(CliError::Usage(_))));
}

#[test]
fn http_requires_service_and_endpoint() {
    assert!(matches!(proxy::http(&[]), Err(CliError::Usage(_))));
    assert!(matches!(
        proxy::http(&["--service".to_string(), "svc".to_string()]),
        Err(CliError::Usage(_))
    ));
}

#[test]
fn vault_list_runs_cleanly() {
    let res = query::list(&["--json".to_string()]);
    assert!(res.is_ok(), "vault list failed: {:?}", res.err());
}

#[test]
fn vault_presets_runs_cleanly() {
    let res = query::presets(&["--json".to_string()]);
    assert!(res.is_ok(), "vault presets failed: {:?}", res.err());
    let output = res.unwrap();
    let val: serde_json::Value = serde_json::from_str(&output).expect("valid JSON array");
    assert!(val.is_array());
    assert!(!val.as_array().unwrap().is_empty());
}

#[test]
fn vault_audit_runs_cleanly() {
    let res = audit::audit(&["--json".to_string()]);
    assert!(res.is_ok(), "vault audit failed: {:?}", res.err());
}
