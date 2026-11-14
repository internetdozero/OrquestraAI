//! Assistant text sanitization shared by every transcript reader.
//!
//! Harnesses record tool runs next to conversation: Antigravity writes
//! `Created At:` / `Output:` / exit-status records into planner responses.
//! Those records are execution metadata, never speech, and must never reach a
//! chat bubble. Split from `transcript.rs` so both files stay under the cap.

/// Harness records of a tool run open with these lines.
const TOOL_RUN_HEADERS: [&str; 2] = ["Created At:", "Completed At:"];
/// Command-output lines that complete a tool-run record.
const TOOL_RUN_OUTPUTS: [&str; 3] = ["Output:", "Stdout:", "Stderr:"];
/// Closing line of a tool-run record.
const TOOL_RUN_EXIT: &str = "The command exited with code";

/// True when this line starts a tool-run record rather than a sentence.
fn opens_tool_run(line: &str) -> bool {
    let line = line.trim_start();
    TOOL_RUN_HEADERS
        .iter()
        .any(|header| line.starts_with(header))
        || TOOL_RUN_OUTPUTS
            .iter()
            .any(|output| line.starts_with(output))
        || line.starts_with(TOOL_RUN_EXIT)
}

/// True when the text carries a complete tool-run signature: a run header with
/// command output or an exit status, or output plus an exit status. A prose
/// sentence that merely starts with `Output:` does not qualify.
fn is_tool_run_record(text: &str) -> bool {
    let mut header = false;
    let mut output = false;
    let mut exit = false;

    for line in text.lines() {
        let line = line.trim_start();
        header |= TOOL_RUN_HEADERS.iter().any(|name| line.starts_with(name));
        output |= TOOL_RUN_OUTPUTS.iter().any(|name| line.starts_with(name));
        exit |= line.contains(TOOL_RUN_EXIT);
    }

    (header && (output || exit)) || (output && exit)
}

/// Strip a harness tool-execution record out of assistant text.
///
/// A record that is the whole message becomes empty, so the turn is dropped;
/// one appended to real planner text is cut at its header so the speech
/// survives. `Created At:` / `Output:` / exit metadata is never conversational
/// speech, and never reaches a chat bubble.
pub(crate) fn strip_tool_run(text: &str) -> String {
    // A message that opens with a run header (even one truncated before its
    // output) is a record. A bare `Output:` opener needs the full signature
    // below, so normal prose starting with the word is left alone.
    let starts_with_record = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| {
            let line = line.trim_start();
            TOOL_RUN_HEADERS
                .iter()
                .any(|header| line.starts_with(header))
                || line.starts_with(TOOL_RUN_EXIT)
        });
    if !starts_with_record && !is_tool_run_record(text) {
        return text.to_string();
    }

    let mut kept: Vec<&str> = Vec::new();
    for line in text.lines() {
        if opens_tool_run(line) {
            break;
        }
        kept.push(line);
    }

    kept.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_cut_at_their_header() {
        let mixed = "I moved the cache.\n\nCreated At: 2026-10-18T11:56:40Z\n\
                     Output:\nls\nThe command exited with code 0";
        assert_eq!(strip_tool_run(mixed), "I moved the cache.");
        assert_eq!(strip_tool_run("Created At: 2026-10-18T11:56:40Z"), "");
        assert_eq!(
            strip_tool_run("Output:\nls\nThe command exited with code 0"),
            ""
        );
    }

    #[test]
    fn prose_is_not_a_record() {
        let prose = "Output: the command prints the resolved path";
        assert_eq!(strip_tool_run(prose), prose);
        assert_eq!(strip_tool_run("no metadata here"), "no metadata here");
    }
}
