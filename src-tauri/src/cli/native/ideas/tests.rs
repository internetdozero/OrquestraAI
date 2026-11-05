use super::*;

#[test]
fn ideas_cli_usage_errors() {
    assert!(matches!(
        run(&["unknown".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(run(&["get".to_string()]), Err(CliError::Usage(_))));
    assert!(matches!(run(&["add".to_string()]), Err(CliError::Usage(_))));
    assert!(matches!(
        run(&["status".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["delete".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["search".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["note-post".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["note-get".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["note-read".to_string()]),
        Err(CliError::Usage(_))
    ));
    assert!(matches!(
        run(&["note-clear".to_string()]),
        Err(CliError::Usage(_))
    ));
}

#[test]
fn ideas_cli_full_lifecycle() {
    let unique_proj = format!(
        "proj-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    // 1. Add idea
    let add_args = vec![
        "add".to_string(),
        "--project".to_string(),
        unique_proj.clone(),
        "--title".to_string(),
        "Lifecycle Task".to_string(),
        "--category".to_string(),
        "feature".to_string(),
        "--priority".to_string(),
        "high".to_string(),
        "--tags".to_string(),
        "test,cli".to_string(),
        "--json".to_string(),
    ];
    let add_out = run(&add_args).expect("add idea");
    let added: serde_json::Value = serde_json::from_str(&add_out).expect("json idea");
    let id = added["id"].as_i64().expect("idea id");
    assert_eq!(added["title"], "Lifecycle Task");

    // 2. Get idea
    let get_args = vec!["get".to_string(), id.to_string(), "--json".to_string()];
    let get_out = run(&get_args).expect("get idea");
    let fetched: serde_json::Value = serde_json::from_str(&get_out).expect("json idea");
    assert_eq!(fetched["id"], id);

    // 3. Update idea
    let update_args = vec![
        "update".to_string(),
        id.to_string(),
        "--title".to_string(),
        "Updated Task".to_string(),
        "--json".to_string(),
    ];
    let update_out = run(&update_args).expect("update idea");
    let updated: serde_json::Value = serde_json::from_str(&update_out).expect("json idea");
    assert_eq!(updated["title"], "Updated Task");

    // 4. Status idea
    let status_args = vec![
        "status".to_string(),
        id.to_string(),
        "completed".to_string(),
        "--json".to_string(),
    ];
    let status_out = run(&status_args).expect("status idea");
    let status_val: serde_json::Value = serde_json::from_str(&status_out).expect("json idea");
    assert_eq!(status_val["status"], "completed");

    // 5. Search
    let search_args = vec![
        "search".to_string(),
        "Updated".to_string(),
        "--project".to_string(),
        unique_proj.clone(),
        "--json".to_string(),
    ];
    let search_out = run(&search_args).expect("search ideas");
    let search_val: serde_json::Value = serde_json::from_str(&search_out).expect("json array");
    assert!(!search_val.as_array().unwrap().is_empty());

    // 6. Projects
    let proj_args = vec!["projects".to_string(), "--json".to_string()];
    let proj_out = run(&proj_args).expect("projects list");
    let proj_val: serde_json::Value = serde_json::from_str(&proj_out).expect("json array");
    assert!(proj_val
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == &unique_proj));

    // 7. Delete
    let del_args = vec!["delete".to_string(), id.to_string(), "--json".to_string()];
    let del_out = run(&del_args).expect("delete idea");
    let del_val: serde_json::Value = serde_json::from_str(&del_out).expect("json");
    assert_eq!(del_val["deleted"], true);
}

#[test]
fn blackboard_notes_cli_lifecycle() {
    let unique_proj = format!(
        "notes-proj-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    // 1. Post note
    let post_args = vec![
        "note-post".to_string(),
        "--project".to_string(),
        unique_proj.clone(),
        "--topic".to_string(),
        "deploy".to_string(),
        "--note".to_string(),
        "Test message".to_string(),
        "--sender".to_string(),
        "agent-1".to_string(),
        "--json".to_string(),
    ];
    let post_out = run(&post_args).expect("post note");
    let posted: serde_json::Value = serde_json::from_str(&post_out).expect("json note");
    let id = posted["id"].as_i64().expect("note id");
    assert_eq!(posted["topic"], "deploy");
    assert_eq!(posted["isRead"], false);

    // 2. Note list
    let list_args = vec![
        "note-list".to_string(),
        "--project".to_string(),
        unique_proj.clone(),
        "--json".to_string(),
    ];
    let list_out = run(&list_args).expect("list notes");
    let list_val: serde_json::Value = serde_json::from_str(&list_out).expect("json array");
    assert_eq!(list_val.as_array().unwrap().len(), 1);

    // 3. Note get
    let get_args = vec!["note-get".to_string(), id.to_string(), "--json".to_string()];
    let get_out = run(&get_args).expect("get note");
    let get_val: serde_json::Value = serde_json::from_str(&get_out).expect("json note");
    assert_eq!(get_val["id"], id);

    // 4. Note read
    let read_args = vec![
        "note-read".to_string(),
        id.to_string(),
        "--json".to_string(),
    ];
    let read_out = run(&read_args).expect("mark read");
    let read_val: serde_json::Value = serde_json::from_str(&read_out).expect("json report");
    assert_eq!(read_val["isRead"], true);

    // 5. Note clear
    let clear_args = vec![
        "note-clear".to_string(),
        "--project".to_string(),
        unique_proj.clone(),
        "--json".to_string(),
    ];
    let clear_out = run(&clear_args).expect("clear notes");
    let clear_val: serde_json::Value = serde_json::from_str(&clear_out).expect("json report");
    assert_eq!(clear_val["cleared"], 1);
}
