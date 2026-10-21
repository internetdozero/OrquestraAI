import type { TranscriptTurn } from "../../lib/sessions";

/** Antigravity: user envelopes already unwrapped, planner text only. */
export const ANTIGRAVITY_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content:
      "Move the transcript sanitizer into its own adapter module and keep the drawer stable.",
    timestamp: "2026-10-18T09:12:00Z",
  },
  {
    role: "assistant",
    content:
      "## Plan\n\n1. Keep the sanitizer behind `clean_turn_content`.\n2. Drop tool-run records before they reach the feed.\n3. Leave the adapter line caps intact.\n\nThe drawer should only ever paint dialogue.",
    timestamp: "2026-10-18T09:12:40Z",
  },
  {
    role: "user",
    content: "Also stop the system prompt from showing up as a bubble in the transcript.",
    timestamp: "2026-10-18T09:13:10Z",
  },
  {
    role: "assistant",
    content:
      "Done. `turn_role` now refuses system and developer rows, so the feed starts at your first real prompt.\n\nTool-run records are cut at their header, which means a planner reply that ran a command shows the reply and nothing else.",
    timestamp: "2026-10-18T09:14:00Z",
  },
];
