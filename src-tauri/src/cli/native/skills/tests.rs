use super::*;

#[test]
fn skills_cli_usage_errors() {
    assert!(matches!(
        run(&["unknown".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["info".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["enable".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["disable".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["batch".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["delete".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["create".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["content".to_string()]),
        Err(CliError::Usage(_))
    ));
}

#[test]
fn skills_cli_list_and_summary() {
    let _home = crate::test_env::isolated_home();
    let list_out = run(&["list".to_string(), "--json".to_string()]).expect("skills list");
    let list_val: serde_json::Value = serde_json::from_str(&list_out).expect("json array");
    assert!(list_val.is_array());

    let summary_out = run(&["summary".to_string(), "--json".to_string()]).expect("skills summary");
    let summary_val: serde_json::Value = serde_json::from_str(&summary_out).expect("json array");
    assert!(summary_val.is_array());
}

#[test]
fn skills_cli_scaffold_lifecycle() {
    let unique_id = format!(
        "test-skill-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    // 1. Create skill
    let create_args = vec![
        "create".to_string(),
        unique_id.clone(),
        "--name".to_string(),
        "Lifecycle Skill".to_string(),
        "--description".to_string(),
        "Testing lifecycle in CLI".to_string(),
        "--with-scripts".to_string(),
        "--json".to_string(),
    ];
    let create_out = run(&create_args).expect("create skill");
    let create_val: serde_json::Value = serde_json::from_str(&create_out).expect("json");
    assert_eq!(create_val["id"], unique_id);

    // 2. Validate skill
    let val_args = vec![
        "validate".to_string(),
        unique_id.clone(),
        "--json".to_string(),
    ];
    let val_out = run(&val_args).expect("validate skill");
    let val_obj: serde_json::Value = serde_json::from_str(&val_out).expect("json");
    assert_eq!(val_obj["valid"], true);

    // 3. Info / inspect skill
    let info_args = vec!["info".to_string(), unique_id.clone(), "--json".to_string()];
    let info_out = run(&info_args).expect("info skill");
    let info_val: serde_json::Value = serde_json::from_str(&info_out).expect("json");
    assert_eq!(info_val["skill"]["id"], unique_id);

    // 4. Enable & disable
    let enable_args = vec![
        "enable".to_string(),
        unique_id.clone(),
        "--scope".to_string(),
        "global".to_string(),
        "--json".to_string(),
    ];
    let enable_out = run(&enable_args).expect("enable skill");
    let enable_val: serde_json::Value = serde_json::from_str(&enable_out).expect("json");
    assert_eq!(enable_val["enabled"], true);

    let disable_args = vec![
        "disable".to_string(),
        unique_id.clone(),
        "--scope".to_string(),
        "global".to_string(),
        "--json".to_string(),
    ];
    let disable_out = run(&disable_args).expect("disable skill");
    let disable_val: serde_json::Value = serde_json::from_str(&disable_out).expect("json");
    assert_eq!(disable_val["enabled"], false);

    // 5. Content get
    let content_out =
        run(&["content".to_string(), "get".to_string(), unique_id.clone()]).expect("content get");
    assert!(content_out.contains("Lifecycle Skill"));

    // 6. Delete skill
    let del_args = vec![
        "delete".to_string(),
        unique_id.clone(),
        "--force".to_string(),
        "--json".to_string(),
    ];
    let del_out = run(&del_args).expect("delete skill");
    let del_val: serde_json::Value = serde_json::from_str(&del_out).expect("json");
    assert_eq!(del_val["deleted"], true);
}
