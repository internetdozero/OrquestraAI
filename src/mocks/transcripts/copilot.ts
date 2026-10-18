import type { TranscriptTurn } from "../../lib/sessions";

/** Copilot: one row per exchange, split into prompt and response. */
export const COPILOT_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content: "Show me the last two turns from the Copilot store.",
    timestamp: "2026-10-18T06:28:00Z",
  },
  {
    role: "assistant",
    content:
      "The `turns` table stores one row per exchange. The reader splits each row into a user turn and an assistant turn, ordered by `turn_index`, and skips empty halves.",
    timestamp: "2026-10-18T06:30:00Z",
  },
  {
    role: "user",
    content: "What happens when a response is still streaming?",
    timestamp: "2026-10-18T06:32:00Z",
  },
  {
    role: "assistant",
    content:
      "The prompt still renders; the assistant half is simply absent until the row is updated. No placeholder text and no error.",
    timestamp: "2026-10-18T06:33:00Z",
  },
];
