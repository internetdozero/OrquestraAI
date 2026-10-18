import type { TranscriptTurn } from "../../lib/sessions";

/** Codex: a multi-turn rollout read back in order. */
export const CODEX_TURNS: TranscriptTurn[] = [
  {
    role: "user",
    content: "Split the rollout reader so each thread pages independently.",
    timestamp: "2026-10-18T09:55:00Z",
  },
  {
    role: "assistant",
    content:
      "I kept `read_transcript` on the adapter and left rollout discovery alone.\n\n- `response_item` rows only\n- `user` and `assistant` payload roles only\n- tool items skipped\n\nThat keeps developer instructions and environment setup out of the transcript.",
    timestamp: "2026-10-18T09:56:20Z",
  },
  {
    role: "user",
    content: "Did you keep the 1000-turn ceiling?",
    timestamp: "2026-10-18T10:01:00Z",
  },
  {
    role: "assistant",
    content:
      "Yes. `MAX_TRANSCRIPT_TURNS` is shared with the Codex multi-file pager, so a long rollout stops at the same cap as every other harness.",
    timestamp: "2026-10-18T10:02:10Z",
  },
  {
    role: "user",
    content: "Good. Run the tests.",
    timestamp: "2026-10-18T10:04:00Z",
  },
  {
    role: "assistant",
    content:
      "`sessions::adapters` is green: 65 passed. The transcript fixtures cover all nine stores now.",
    timestamp: "2026-10-18T10:05:00Z",
  },
];
