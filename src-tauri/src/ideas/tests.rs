#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Unit tests against an in-memory vault: no scratch files, no HOME needed.

use std::path::Path;

use rusqlite::Connection;

use super::{ideas_repo, notes_repo, productivity, store};
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
            prompt: None,
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
            prompt: None,
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
            prompt: None,
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
fn prompt_roundtrips_through_create_update_and_list() {
    let connection = memory();
    let created = ideas_repo::create_idea(
        &connection,
        CreateIdeaInput {
            project: "vault".to_string(),
            title: "Prompted task".to_string(),
            description: Some("short summary".to_string()),
            category: None,
            priority: None,
            tags: None,
            prompt: Some("### Mission: Prompted task\n\nDo the thing.".to_string()),
        },
    )
    .expect("create with prompt");
    assert_eq!(
        created.prompt.as_deref(),
        Some("### Mission: Prompted task\n\nDo the thing.")
    );

    let listed = ideas_repo::list_ideas(&connection, Some("vault"), Some("all"))
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].prompt, created.prompt);

    let updated = ideas_repo::update_idea(
        &connection,
        UpdateIdeaInput {
            id: created.id,
            prompt: Some("Revised builder dispatch".to_string()),
            ..UpdateIdeaInput::default()
        },
    )
    .expect("update prompt");
    assert_eq!(updated.prompt.as_deref(), Some("Revised builder dispatch"));

    // A blank prompt clears back to None so the UI can show "no prompt".
    let cleared = ideas_repo::update_idea(
        &connection,
        UpdateIdeaInput {
            id: created.id,
            prompt: Some("   ".to_string()),
            ..UpdateIdeaInput::default()
        },
    )
    .expect("clear prompt");
    assert_eq!(cleared.prompt, None);
}

#[test]
fn completed_at_stamps_completions_and_clears_on_reopen() {
    let connection = memory();
    let idea = create(&connection, "vault", "Ship it", None);
    assert_eq!(idea.completed_at, None, "open ideas carry no stamp");

    let done =
        ideas_repo::update_idea_status(&connection, idea.id, "completed").expect("complete");
    assert!(
        done.completed_at.is_some(),
        "completing must stamp completed_at"
    );

    let archived =
        ideas_repo::update_idea_status(&connection, idea.id, "archived").expect("archive");
    assert_eq!(
        archived.completed_at, done.completed_at,
        "archiving keeps the delivery stamp"
    );

    let reopened =
        ideas_repo::update_idea_status(&connection, idea.id, "open").expect("reopen");
    assert_eq!(
        reopened.completed_at, None,
        "reopening must clear the stamp"
    );
}

#[test]
fn legacy_vaults_gain_completed_at_without_losing_rows() {
    let path = std::env::temp_dir().join(format!("orquestra-legacy-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    // A pre-migration table: no completed_at column at all.
    {
        let legacy = Connection::open(&path).expect("legacy db should open");
        legacy
            .execute_batch(
                "CREATE TABLE ideas (id INTEGER PRIMARY KEY AUTOINCREMENT, \
                 project TEXT NOT NULL, title TEXT NOT NULL, description TEXT, \
                 category TEXT NOT NULL DEFAULT 'feature', \
                 priority TEXT NOT NULL DEFAULT 'medium', tags TEXT, \
                 status TEXT NOT NULL DEFAULT 'open', \
                 created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                 updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP); \
                 INSERT INTO ideas (project, title) VALUES ('vault', 'Legacy row');",
            )
            .expect("legacy schema should apply");
    }
    let connection = store::connect_at(&path).expect("migration should open");
    let ideas = ideas_repo::list_ideas(&connection, Some("vault"), Some("all"))
        .expect("legacy rows stay readable");
    assert_eq!(ideas.len(), 1);
    assert_eq!(ideas[0].completed_at, None);
    // Second open must be a no-op, not a duplicate-column error.
    store::connect_at(&path).expect("migration must be idempotent");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn productivity_counts_completions_streak_and_cycle() {
    let connection = memory();
    let first = create(&connection, "vault", "First", None);
    let second = create(&connection, "vault", "Second", None);
    ideas_repo::update_idea_status(&connection, first.id, "completed").expect("complete");
    ideas_repo::update_idea_status(&connection, second.id, "completed").expect("complete");

    let metrics = productivity::metrics(&connection, Some("vault"), Some(2))
        .expect("metrics should compute");
    assert_eq!(metrics.daily_activity.len(), 14);
    let today = metrics.daily_activity.last().expect("window ends today");
    assert_eq!(today.ideas_completed, 2, "both completions land today");
    assert_eq!(today.intensity, 2, "two deliveries is a steady day");
    assert!(metrics.current_streak >= 1, "today is active");
    assert_eq!(metrics.weekly_velocity.this_week, 2);
    assert_eq!(metrics.weekly_velocity.last_week, 0);
    assert!(
        metrics.avg_cycle_hours.unwrap_or(-1.0) >= 0.0,
        "cycle time from creation to completion"
    );

    // An unknown project has no deliveries: empty, never an error.
    let scoped = productivity::metrics(&connection, Some("no-such-project"), Some(1))
        .expect("unknown project must not fail");
    assert_eq!(scoped.daily_activity.len(), 7);
    assert!(scoped.daily_activity.iter().all(|day| day.ideas_completed == 0));
    assert_eq!(scoped.current_streak, 0);
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
