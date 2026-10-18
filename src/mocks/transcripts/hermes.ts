import type { TranscriptTurn } from "../../lib/sessions";

/** Hermes: active messages only, archived drafts excluded. */
export const HERMES_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content: "Triage the failed migration job and keep the transcript readable.",
    timestamp: "2026-10-18T05:41:00Z",
  },
  {
    role: "assistant",
    content:
      "The job failed on a lock timeout, not on the schema change.\n\n1. Retry after the writer drains.\n2. Keep the transcript to active messages only.",
    timestamp: "2026-10-18T05:43:00Z",
  },
  {
    role: "user",
    content: "What about the archived drafts?",
    timestamp: "2026-10-18T05:45:00Z",
  },
  {
    role: "assistant",
    content:
      "They stay out of the feed: rows with `active = 0` are skipped, and epoch-float timestamps are normalized to ISO 8601 for display.",
    timestamp: "2026-10-18T05:47:00Z",
  },
];
