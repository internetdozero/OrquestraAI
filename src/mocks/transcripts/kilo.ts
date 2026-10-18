import type { TranscriptTurn } from "../../lib/sessions";

/** Kilo: code parts extracted, diff metadata left on disk. */
export const KILO_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content: "Extract the code part from the Kilo transcript without the diff noise.",
    timestamp: "2026-10-18T03:08:00Z",
  },
  {
    role: "assistant",
    content:
      "The reader now prefers `text` parts and ignores tool payloads, so a code-heavy reply keeps its prose and code while the diff metadata stays on disk.",
    timestamp: "2026-10-18T03:11:00Z",
  },
  {
    role: "user",
    content: "Does the search still match inside code?",
    timestamp: "2026-10-18T03:14:00Z",
  },
  {
    role: "assistant",
    content:
      "Yes. Search runs over the final normalized content, so fenced code is searchable just like prose.",
    timestamp: "2026-10-18T03:15:00Z",
  },
];
