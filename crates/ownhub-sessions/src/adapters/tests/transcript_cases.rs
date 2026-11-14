//! Transcript readers: what a feed shows and what it deliberately drops.

use super::{adapter, TestHome};
use crate::adapters::HARNESS_IDS;

#[test]
fn claude_transcript_reads_turns_and_skips_noise() {
    let home = TestHome::new();
    home.write(
        ".claude/projects/-work-app/abc123.jsonl",
        "{\"type\":\"mode\",\"sessionId\":\"abc123\"}\n\
         {\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\
          [{\"type\":\"text\",\"text\":\"first prompt\"}]},\
          \"timestamp\":\"2026-09-20T23:46:04.449Z\"}\n\
         this line is not json\n\
         {\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\
          [{\"type\":\"tool_result\",\"tool_use_id\":\"t1\",\"content\":\"huge dump\"}]}}\n\
         {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":\
          [{\"type\":\"text\",\"text\":\"here you go\"},\
           {\"type\":\"tool_use\",\"name\":\"Bash\",\"input\":{}}]},\
          \"timestamp\":\"2026-09-20T23:46:10.000Z\"}\n\
         {\"type\":\"summary\",\"summary\":\"compacted\"}\n",
    );

    let turns = adapter(&home.0, "claude").read_transcript("abc123");
    assert_eq!(
        turns.len(),
        2,
        "mode, corrupt, tool-only and summary lines are skipped"
    );
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "first prompt");
    assert_eq!(turns[0].timestamp.as_deref(), Some("2026-09-20T23:46:04Z"));
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "here you go");

    assert!(
        adapter(&home.0, "claude")
            .read_transcript("missing")
            .is_empty(),
        "no transcript file is an empty feed, not an error"
    );
}

#[test]
fn antigravity_transcript_covers_both_log_layouts() {
    let home = TestHome::new();
    home.write(
        ".gemini/antigravity/brain/a8ceb54b/logs/transcript.jsonl",
        "{\"role\":\"user\",\"content\":\"fix the kernel\",\
          \"timestamp\":\"2026-08-18T12:11:33Z\"}\n\
         {\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"on it\"}]}\n\
         {\"unrelated\":\"shape\"}\n",
    );
    home.write(
        ".gemini/antigravity-cli/brain/c1/.system_generated/logs/transcript.jsonl",
        "{\"role\":\"user\",\"content\":\"cli hello\"}\n",
    );

    let turns = adapter(&home.0, "antigravity").read_transcript("a8ceb54b");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[0].content, "fix the kernel");
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].content, "on it");

    let cli = adapter(&home.0, "antigravity").read_transcript("c1");
    assert_eq!(cli.len(), 1);
    assert_eq!(cli[0].content, "cli hello");

    assert!(adapter(&home.0, "antigravity")
        .read_transcript("missing")
        .is_empty());
}

#[test]
fn harnesses_without_data_report_an_empty_feed() {
    // Every harness has a transcript reader now; with no store on disk each
    // one must report an empty feed rather than an error. Readers with
    // fixture data are covered in `transcript_tests`.
    let home = TestHome::new();
    for harness in HARNESS_IDS {
        assert!(
            adapter(&home.0, harness)
                .read_transcript("whatever")
                .is_empty(),
            "{harness} must return an empty feed when its store is missing"
        );
    }
}
