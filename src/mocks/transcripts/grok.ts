import type { TranscriptTurn } from "../../lib/sessions";

/** Grok: no system prompt dump, just the exchange. */
export const GROK_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content: "The drawer opens with a wall of harness system instructions. Why?",
    timestamp: "2026-10-18T08:38:00Z",
  },
  {
    role: "assistant",
    content:
      "That was the harness system prompt being treated as a transcript turn.\n\nThe reader now skips `system` and `developer` roles entirely, so the feed opens with your first question instead.",
    timestamp: "2026-10-18T08:39:12Z",
  },
  {
    role: "user",
    content: "And the transcript search?",
    timestamp: "2026-10-18T08:40:00Z",
  },
  {
    role: "assistant",
    content:
      "Search stays client-side over the parsed turns: it matches content only, ignores role labels, and never re-reads the session file.",
    timestamp: "2026-10-18T08:41:00Z",
  },
];
