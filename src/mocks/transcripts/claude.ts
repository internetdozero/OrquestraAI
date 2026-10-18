import type { TranscriptTurn } from "../../lib/sessions";

/** Claude: clean Markdown, no scaffolding rows. */
export const CLAUDE_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content: "Summarize what changed in the transcript pipeline this week.",
    timestamp: "2026-10-18T11:12:00Z",
  },
  {
    role: "assistant",
    content:
      "## Transcript pipeline\n\n**Sanitization**\n- system prompts are skipped\n- tool-run records are stripped\n- thinking blocks stay removed\n\n**Fixtures**\n- one gallery covers all nine harnesses\n\n> The exported Markdown mirrors the visible feed.\n\n_What you review is what you copy._",
    timestamp: "2026-10-18T11:14:30Z",
  },
  {
    role: "user",
    content: "Any risk to the IPC contract?",
    timestamp: "2026-10-18T11:18:00Z",
  },
  {
    role: "assistant",
    content:
      "None. `TranscriptTurn` keeps the same three fields; only the values of `role` narrow to `user` and `assistant`.",
    timestamp: "2026-10-18T11:20:00Z",
  },
];
