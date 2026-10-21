import type { TranscriptTurn } from "../../lib/sessions";

/** OpenCode: message role joined to text parts, oldest first. */
export const OPENCODE_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content: "The part table has text and tool rows. Which ones should the drawer show?",
    timestamp: "2026-10-18T04:12:00Z",
  },
  {
    role: "assistant",
    content:
      'Only text parts:\n\n```ts\nconst parts = rows.filter((part) => part.type === "text");\n```\n\nThe join keeps message roles authoritative, so a tool part never becomes an assistant bubble.',
    timestamp: "2026-10-18T04:14:00Z",
  },
  {
    role: "user",
    content: "And when the join is unavailable?",
    timestamp: "2026-10-18T04:18:00Z",
  },
  {
    role: "assistant",
    content:
      "The reader falls back to the `message` table, then to `part` rows that carry their own role. Same normalized transcript, three schema shapes.",
    timestamp: "2026-10-18T04:20:00Z",
  },
];
