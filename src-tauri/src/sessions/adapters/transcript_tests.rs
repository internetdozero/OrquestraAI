#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Transcript-reader fixtures: one test per harness store layout.
//!
//! Each test builds the real on-disk shape (JSONL log, SQLite database or
//! JSON chat log) under a throwaway home, reads through the adapter's
//! `read_transcript`, and asserts the drawer sees clean turns. Listing
//! coverage stays in `tests.rs`; this module only covers conversation reads.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::{registry, SessionAdapter};

struct TestHome(PathBuf);

impl TestHome {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-transcripts-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create the transcript test home under /var/tmp/testing");
        Self(path)
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create the fixture parent");
        fs::write(&path, content).expect("write the fixture file");
    }

    fn database(&self, relative: &str, schema: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create the fixture parent");
        Connection::open(path)
            .expect("open the fixture database")
            .execute_batch(schema)
            .expect("apply the fixture schema");
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn adapter(home: &Path, harness: &str) -> Box<dyn SessionAdapter> {
    registry(home)
        .into_iter()
        .find(|adapter| adapter.harness_id() == harness)
        .unwrap_or_else(|| panic!("no adapter registered for {harness}"))
}

#[test]
fn antigravity_unwraps_user_requests_and_planner_responses() {
    let home = TestHome::new("antigravity");
    home.write(
        ".gemini/antigravity/brain/sess-1/.system_generated/logs/transcript.jsonl",
        "{\"source\":\"USER_EXPLICIT\",\"type\":\"USER_INPUT\",\
          \"content\":\"<USER_REQUEST>move the cache</USER_REQUEST>\",\
          \"timestamp\":\"2026-08-18T12:11:33.083102087+00:00\"}\n\
         {\"source\":\"MODEL\",\"type\":\"PLANNER_RESPONSE\",\
          \"content\":[{\"type\":\"text\",\"text\":\"moved <thinking>quiet</thinking> it\"}],\
          \"timestamp\":\"2026-08-18T12:12:01+00:00\"}\n\
         {\"source\":\"MODEL\",\"type\":\"TOOL_CALL\",\"tool\":\"Bash\",\"args\":{\"cmd\":\"ls\"}}\n",
    );

    let turns = adapter(&home.0, "antigravity").read_transcript("sess-1");
    assert_eq!(turns.len(), 2, "tool calls are not conversation: {turns:?}");
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "move the cache");
    assert_eq!(
        turns[0].timestamp.as_deref(),
        Some("2026-08-18T12:11:33Z")
    );
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "moved  it");
}

#[test]
fn grok_reads_chat_history_in_any_project_folder() {
    let home = TestHome::new("grok");
    home.write(
        ".grok/sessions/-work-app/sess-9/chat_history.jsonl",
        "{\"type\":\"user\",\"content\":\"what changed?\",\"timestamp\":1791708904}\n\
         {\"type\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"three files\"}]}\n\
         not json at all\n",
    );

    let turns = adapter(&home.0, "grok").read_transcript("sess-9");
    assert_eq!(turns.len(), 2, "corrupt lines are skipped: {turns:?}");
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "what changed?");
    assert_eq!(
        turns[0].timestamp.as_deref(),
        Some("2026-10-11T08:55:04Z")
    );
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "three files");

    assert!(
        adapter(&home.0, "grok").read_transcript("../escape").is_empty(),
        "session ids never escape the store"
    );
}

#[test]
fn codex_reads_response_items_and_ignores_scaffolding() {
    let home = TestHome::new("codex");
    let id = "01a148f4-9277-7820-8e2c-30f21ca5100f";
    home.write(
        &format!(".codex/sessions/2026/10/17/rollout-x-{id}.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/work\"}}\n\
         {\"type\":\"response_item\",\"timestamp\":\"2026-10-17T05:22:31Z\",\
          \"payload\":{\"role\":\"developer\",\"content\":[{\"type\":\"input_text\",\"text\":\"be nice\"}]}}\n\
         {\"type\":\"response_item\",\"timestamp\":\"2026-10-17T05:22:32Z\",\
          \"payload\":{\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"ship it\"}]}}\n\
         {\"type\":\"response_item\",\
          \"payload\":{\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"shipped\"},\
          {\"type\":\"tool_use\",\"name\":\"Bash\",\"input\":{}}]}}\n",
    );

    let turns = adapter(&home.0, "codex").read_transcript(id);
    assert_eq!(turns.len(), 2, "developer prompts are scaffolding: {turns:?}");
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "ship it");
    assert_eq!(
        turns[0].timestamp.as_deref(),
        Some("2026-10-17T05:22:32Z")
    );
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "shipped");
}

#[test]
fn copilot_splits_rows_into_user_and_assistant_turns() {
    let home = TestHome::new("copilot");
    home.database(
        ".copilot/session-store.db",
        "CREATE TABLE turns (session_id TEXT, turn_index INTEGER, user_message TEXT,\
                            assistant_response TEXT, timestamp TEXT);\n\
         INSERT INTO turns VALUES ('s1', 1, 'second q', 'second a', '2026-09-18T03:17:00Z');\n\
         INSERT INTO turns VALUES ('s1', 0, 'first q', '', '2026-09-18T03:16:00Z');\n\
         INSERT INTO turns VALUES ('other', 0, 'nope', 'nope', '2026-09-18T03:16:00Z');",
    );

    let turns = adapter(&home.0, "copilot").read_transcript("s1");
    assert_eq!(turns.len(), 3, "turn_index orders, empties are skipped: {turns:?}");
    assert_eq!((turns[0].role.as_str(), turns[0].content.as_str()), ("user", "first q"));
    assert_eq!((turns[1].role.as_str(), turns[1].content.as_str()), ("user", "second q"));
    assert_eq!(
        (turns[2].role.as_str(), turns[2].content.as_str()),
        ("assistant", "second a")
    );
    assert_eq!(
        turns[0].timestamp.as_deref(),
        Some("2026-09-18T03:16:00Z")
    );
}

#[test]
fn hermes_reads_active_messages_with_epoch_floats() {
    let home = TestHome::new("hermes");
    home.database(
        ".hermes/state.db",
        "CREATE TABLE messages (id INTEGER PRIMARY KEY, session_id TEXT, role TEXT,\
                                content TEXT, timestamp REAL, active INTEGER);\n\
         INSERT INTO messages VALUES (1, 'h1', 'user', 'triage this', 1787055093.0, 1);\n\
         INSERT INTO messages VALUES (2, 'h1', 'assistant', 'on it', 1787055153.0, NULL);\n\
         INSERT INTO messages VALUES (3, 'h1', 'assistant', 'archived draft', 1787055200.0, 0);\n\
         INSERT INTO messages VALUES (4, 'h1', 'system', 'prompt', 1787055200.0, 1);",
    );

    let turns = adapter(&home.0, "hermes").read_transcript("h1");
    assert_eq!(
        turns.len(),
        2,
        "archived and system rows are not conversation: {turns:?}"
    );
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "triage this");
    assert_eq!(
        turns[0].timestamp.as_deref(),
        Some("2026-08-18T12:11:33Z")
    );
    assert_eq!(turns[1].role, "assistant");
}

#[test]
fn opencode_and_kilo_read_part_payloads_through_their_messages() {
    for store in ["opencode", "kilo"] {
        let home = TestHome::new(store);
        home.database(
            &format!(".local/share/{store}/{store}.db"),
            "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, role TEXT);\n\
             CREATE TABLE part (id TEXT PRIMARY KEY, session_id TEXT, message_id TEXT,\
                               data TEXT, time_created INTEGER);\n\
             INSERT INTO message VALUES ('m1', 'ses_1', 'user'), ('m2', 'ses_1', 'assistant');\n\
             INSERT INTO part VALUES\n               ('p1', 'ses_1', 'm1', '{\"type\":\"text\",\"text\":\"refactor this\"}', 100),\n\
               ('p2', 'ses_1', 'm2', '{\"type\":\"tool\",\"input\":{}}', 200),\n\
               ('p3', 'ses_1', 'm2', '{\"type\":\"text\",\"text\":\"done\"}', 300);",
        );

        let turns = adapter(&home.0, store).read_transcript("ses_1");
        assert_eq!(turns.len(), 2, "{store}: tool parts are skipped: {turns:?}");
        assert_eq!(turns[0].role, "user");
        assert_eq!(turns[0].content, "refactor this");
        assert_eq!(turns[1].role, "assistant");
        assert_eq!(turns[1].content, "done");
    }
}

#[test]
fn manicode_chat_logs_read_as_json_arrays() {
    let home = TestHome::new("manicode");
    home.write(
        ".config/manicode/projects/app/chats/chat-1/chat.json",
        "{\"title\":\"Deploy\"}",
    );
    home.write(
        ".config/manicode/projects/app/chats/chat-1/chat-messages.json",
        "[{\"role\":\"user\",\"content\":\"deploy now\"},\
          {\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"deploying\"}]},\
          {\"role\":\"system\",\"content\":\"key\"}]",
    );

    let turns = adapter(&home.0, "freebuff").read_transcript("app/chat-1");
    assert_eq!(
        turns.len(),
        2,
        "system scaffolding is not a turn: {turns:?}"
    );
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "deploy now");
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "deploying");
}

#[test]
fn antigravity_tool_execution_dumps_are_not_speech() {
    let home = TestHome::new("antigravity-tools");
    home.write(
        ".gemini/antigravity/brain/sess-2/.system_generated/logs/transcript.jsonl",
        "{\"source\":\"USER_EXPLICIT\",\"type\":\"USER_INPUT\",\
          \"content\":\"<USER_REQUEST>move the cache</USER_REQUEST>\"}\n\
         {\"source\":\"MODEL\",\"type\":\"PLANNER_RESPONSE\",\
          \"content\":\"Created At: 2026-10-18T11:56:40Z\\nCompleted At: 2026-10-18T11:56:41Z\\nOutput:\\nsrc-tauri/src/main.rs\\nThe command exited with code 0\"}\n\
         {\"source\":\"MODEL\",\"type\":\"PLANNER_RESPONSE\",\
          \"content\":[{\"type\":\"text\",\"text\":\"The cache now lives in the new module.\"}]}\n",
    );

    let turns = adapter(&home.0, "antigravity").read_transcript("sess-2");
    assert_eq!(turns.len(), 2, "tool runs are metadata: {turns:?}");
    assert_eq!(turns[0].content, "move the cache");
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "The cache now lives in the new module.");
}

#[test]
fn grok_sanitizes_environment_rules_and_extracts_user_query() {
    let home = TestHome::new("grok-user-query");
    home.write(
        ".grok/sessions/-work-app/sess-7/chat_history.jsonl",
        "{\"type\":\"user\",\"synthetic_reason\":\"system_reminder\",\
          \"content\":\"<user_info>cwd: /work/app</user_info>\\n<rules>be terse</rules>\"}\n\
         {\"type\":\"user\",\
          \"content\":\"<user_info>cwd: /work/app</user_info>\\n<rules>be terse</rules>\"}\n\
         {\"type\":\"user\",\
          \"content\":\"<system-reminder>context</system-reminder>\\n<user_query>\\nfix the drawer\\n</user_query>\",\
          \"timestamp\":1791708904}\n\
         {\"type\":\"assistant\",\
          \"content\":[{\"type\":\"text\",\"text\":\"Fixed the **drawer**.\"}]}\n\
         {\"type\":\"assistant\",\"synthetic_reason\":\"task_completed\",\
          \"content\":\"task finished\"}\n",
    );

    let turns = adapter(&home.0, "grok").read_transcript("sess-7");
    assert_eq!(
        turns.len(),
        2,
        "rules and synthetic rows are scaffolding: {turns:?}"
    );
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "fix the drawer");
    assert_eq!(
        turns[0].timestamp.as_deref(),
        Some("2026-10-11T08:55:04Z")
    );
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "Fixed the **drawer**.");
    assert!(
        turns
            .iter()
            .all(|turn| !turn.content.contains("<rules>")
                && !turn.content.contains("<user_info>")
                && !turn.content.contains("system-reminder")),
        "no environment scaffolding anywhere: {turns:?}"
    );
}

#[test]
fn system_prompt_rows_never_reach_the_feed() {
    let home = TestHome::new("grok-system");
    home.write(
        ".grok/sessions/-work-app/sess-1/chat_history.jsonl",
        "{\"type\":\"system\",\"content\":\"You are Grok 4.7\\n<dangerous_actions>never run rm -rf</dangerous_actions>\"}\n\
         {\"type\":\"user\",\"content\":\"summarize the diff\"}\n\
         {\"type\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Two files changed.\"}]}\n",
    );

    let turns = adapter(&home.0, "grok").read_transcript("sess-1");
    assert_eq!(turns.len(), 2, "the system prompt is scaffolding: {turns:?}");
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[1].role, "assistant");
    assert!(
        turns.iter().all(|turn| !turn.content.contains("dangerous_actions")),
        "no raw system instructions anywhere: {turns:?}"
    );
}

#[test]
fn desktop_threads_read_parts_json_in_seq_order() {
    let home = TestHome::new("desktop");
    let thread = "11111111-2222-3333-4444-555555555555";
    home.write(
        ".config/freebuff-desktop/projects/OrquestraAI-abc123/project.json",
        "{\"projectId\":\"abc123\",\"projectPath\":\"/work/orquestra\"}",
    );
    home.database(
        ".config/freebuff-desktop/projects/OrquestraAI-abc123/desktop-v2.db",
        "CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT);\n\
         CREATE TABLE messages (seq INTEGER PRIMARY KEY AUTOINCREMENT, thread_id TEXT,\
                                role TEXT, parts_json TEXT, ts INTEGER);\n\
         INSERT INTO threads VALUES ('11111111-2222-3333-4444-555555555555', 'T');\n\
         INSERT INTO messages (thread_id, role, parts_json, ts) VALUES\n           ('11111111-2222-3333-4444-555555555555', 'user',\n            '[{\"type\":\"text\",\"text\":\"first\"}]', 1792324600000),\n           ('11111111-2222-3333-4444-555555555555', 'assistant',\n            '[{\"type\":\"text\",\"text\":\"second\"}]', 1792324544000);",
    );

    let turns = adapter(&home.0, "freebuff")
        .read_transcript(&format!("desktop:OrquestraAI-abc123:{thread}"));
    assert_eq!(turns.len(), 2, "seq orders the feed, not the clock: {turns:?}");
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "first");
    assert_eq!(
        turns[0].timestamp.as_deref(),
        Some("2026-10-18T11:56:40Z")
    );
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "second");
    assert_eq!(
        turns[1].timestamp.as_deref(),
        Some("2026-10-18T11:55:44Z")
    );
}
