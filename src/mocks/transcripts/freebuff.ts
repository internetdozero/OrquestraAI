import type { TranscriptTurn } from "../../lib/sessions";

/** Freebuff: desktop thread text parts, no JSON or reasoning parts. */
export const FREEBUFF_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content: "The desktop thread shows raw JSON parts in the assistant bubble.",
    timestamp: "2026-10-18T07:52:00Z",
  },
  {
    role: "assistant",
    content:
      "The `parts_json` payload is now unwrapped before rendering: only `text` parts contribute, while `reasoning`, `tool` and `ad` parts are skipped.\n\nPlain-string rows still pass through unchanged.",
    timestamp: "2026-10-18T07:54:10Z",
  },
  {
    role: "user",
    content: "So a thread with reasoning blocks shows just the answer?",
    timestamp: "2026-10-18T07:57:00Z",
  },
  {
    role: "assistant",
    content:
      "Exactly. Reasoning never leaves the reader, and the transcript keeps sequence order from the `seq` column.",
    timestamp: "2026-10-18T07:58:00Z",
  },
];
