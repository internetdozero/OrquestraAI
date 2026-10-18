//! Unit tests against an in-memory vault: no scratch files, no HOME needed.

use std::path::Path;

use rusqlite::Connection;

use super::{ideas_repo, notes_repo, store};
use super::types::{CreateIdeaInput, PostNoteInput, UpdateIdeaInput};

fn memory() -> Connection {
    store::connect_at(Path::new(":memory:")).expect("in-memory vault should open")
}

fn create(
    connection: &Connection,
    project: &str,
    title: &str,
    priority: Option<&str>,
) -> super::types::IdeaItem {
    ideas_repo::create_idea(
        connection,
        CreateIdeaInput {
            project: project.to_string(),
            title: title.to_string(),
            description: Some("details".to_string()),
            category: None,
            priority: priority.map(str::to_string),
            tags: Some("[\"rust\"]".to_string()),
        },
    )
    .expect("idea should be created")
}

#[test]
fn ideas_default_enums_and_reject_bad_values() {
    let connection = memory();
    let idea = create(&connection, "vault", "Native module", None);

    assert_eq!(idea.id, 1);
    assert_eq!(idea.category, "feature");
    assert_eq!(idea.priority, "medium");
    assert_eq!(idea.status, "open");

    let bad = ideas_repo::create_idea(
        &connection,
        CreateIdeaInput {
            project: "vault".to_string(),
            title: "Bad".to_string(),
            description: None,
            category: Some("nope".to_string()),
            priority: None,
            tags: None,
        },
    );
    assert!(bad.is_err(), "an unknown category must be rejected");

    let blank = ideas_repo::create_idea(
        &connection,
        CreateIdeaInput {
            project: "vault".to_string(),
            title: "   ".to_string(),
            description: None,
            category: None,
            priority: None,
            tags: None,
        },
    );
    assert!(blank.is_err(), "a blank title must be rejected");
}

#[test]
fn ideas_list_orders_by_priority_then_newest() {
    let connection = memory();
    create(&connection, "vault", "Low first", Some("low"));
    create(&connection, "vault", "Critical wins", Some("critical"));
    create(&connection, "vault", "Medium middle", None);

    let ideas = ideas_repo::list_ideas(&connection, Some("vault"), None).expect("list");
    let titles: Vec<_> = ideas.iter().map(|idea| idea.title.as_str()).collect();
    assert_eq!(titles, vec!["Critical wins", "Medium middle", "Low first"]);

    // The default status filter is `open`; `"all"` disables it.
    ideas_repo::update_idea_status(&connection, ideas[0].id, "completed").expect("complete");
    let open = ideas_repo::list_ideas(&connection, Some("vault"), None).expect("open");
    assert_eq!(open.len(), 2);
    let all = ideas_repo::list_ideas(&connection, Some("vault"), Some("all")).expect("all");
    assert_eq!(all.len(), 3);
}

#[test]
fn ideas_update_partially_and_delete_missing_fails() {
    let connection = memory();
    let idea = create(&connection, "vault", "Draft", None);

    let updated = ideas_repo::update_idea(
        &connection,
        UpdateIdeaInput {
            id: idea.id,
            status: Some("in_progress".to_string()),
            priority: Some("HIGH".to_string()),
            ..UpdateIdeaInput::default()
        },
    )
    .expect("update");
    assert_eq!(updated.status, "in_progress");
    assert_eq!(updated.priority, "high", "choices are normalized");
    assert_eq!(updated.title, "Draft", "untouched fields survive");

    let bad = ideas_repo::update_idea_status(&connection, idea.id, "shipped");
    assert!(bad.is_err(), "an unknown status must be rejected");

    ideas_repo::delete_idea(&connection, idea.id).expect("delete");
    assert!(
        ideas_repo::delete_idea(&connection, idea.id).is_err(),
        "deleting twice must fail loudly"
    );
}

#[test]
fn projects_list_distinct_names_sorted() {
    let connection = memory();
    create(&connection, "zeta", "Z", None);
    create(&connection, "alpha", "A", None);
    create(&connection, "zeta", "Z2", None);

    assert_eq!(
        ideas_repo::list_idea_projects(&connection).expect("projects"),
        vec!["alpha".to_string(), "zeta".to_string()]
    );
}

#[test]
fn blackboard_posts_lists_unread_and_marks_read() {
    let connection = memory();
    let note = notes_repo::post_note(
        &connection,
        PostNoteInput {
            project: "vault".to_string(),
            topic: "Handoff".to_string(),
            sender: "builder".to_string(),
            target: None,
            note: "Phase 1 is green.".to_string(),
        },
    )
    .expect("post");

    assert_eq!(note.topic, "handoff", "topics are lowercased");
    assert_eq!(note.target, "all");
    assert!(!note.is_read);

    let unread = notes_repo::list_notes(&connection, Some("vault"), true).expect("unread");
    assert_eq!(unread.len(), 1);

    notes_repo::mark_note_read(&connection, note.id).expect("mark read");
    assert!(
        notes_repo::list_notes(&connection, Some("vault"), true)
            .expect("reread")
            .is_empty()
    );
    let all = notes_repo::list_notes(&connection, None, false).expect("all notes");
    assert_eq!(all.len(), 1);
    assert!(all[0].is_read);

    assert!(
        notes_repo::mark_note_read(&connection, 999).is_err(),
        "an unknown note must fail"
    );
}

#[test]
fn database_paths_honor_env_and_xdg() {
    // Env override wins verbatim, without touching the filesystem.
    std::env::set_var("IDEA_VAULT_DB_PATH", "/var/tmp/testing/vault.sqlite3");
    assert_eq!(
        store::resolve_db_path().expect("override"),
        Path::new("/var/tmp/testing/vault.sqlite3")
    );
    std::env::remove_var("IDEA_VAULT_DB_PATH");

    std::env::set_var("XDG_DATA_HOME", "/var/tmp/testing/xdg");
    assert_eq!(
        store::resolve_db_path().expect("xdg"),
        Path::new("/var/tmp/testing/xdg/idea-vault/ideas.sqlite3")
    );
    std::env::remove_var("XDG_DATA_HOME");
}
