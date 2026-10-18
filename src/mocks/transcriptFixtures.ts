import type { TranscriptTurn, UnifiedSession } from "../lib/sessions";
import { ANTIGRAVITY_TURNS } from "./transcripts/antigravity";
import { CLAUDE_TURNS } from "./transcripts/claude";
import { CODEX_TURNS } from "./transcripts/codex";
import { COPILOT_TURNS } from "./transcripts/copilot";
import { FREEBUFF_TURNS } from "./transcripts/freebuff";
import { GROK_TURNS } from "./transcripts/grok";
import { HERMES_TURNS } from "./transcripts/hermes";
import { KILO_TURNS } from "./transcripts/kilo";
import { OPENCODE_TURNS } from "./transcripts/opencode";

/**
 * Developer fixture gallery: representative multi-turn conversations for all
 * nine harnesses, so the Session Detail Drawer can be reviewed without nine
 * live agents installed. Data only — activation lives in `lib/fixtures`.
 *
 * The turns are what the readers should emit *after* sanitization: no system
 * prompts, no tool-run records, no reasoning blocks, no raw JSON parts.
 */

interface Fixture {
  harness: string;
  id: string;
  title: string;
  projectPath: string;
  createdAt: string;
  updatedAt: string;
  sizeBytes: number;
  pinned?: boolean;
  turns: TranscriptTurn[];
}

const FIXTURES: Fixture[] = [
  {
    harness: "antigravity",
    id: "fixture-antigravity",
    title: "Sanitize the transcript adapter",
    projectPath: "/home/operator/work/orquestra",
    createdAt: "2026-10-18T09:12:00Z",
    updatedAt: "2026-10-18T09:14:00Z",
    sizeBytes: 48_320,
    turns: ANTIGRAVITY_TURNS,
  },
  {
    harness: "grok",
    id: "fixture-grok",
    title: "System prompt in the drawer",
    projectPath: "/home/operator/work/orquestra",
    createdAt: "2026-10-18T08:38:00Z",
    updatedAt: "2026-10-18T08:41:00Z",
    sizeBytes: 31_744,
    turns: GROK_TURNS,
  },
  {
    harness: "codex",
    id: "fixture-codex",
    title: "Page Codex rollouts independently",
    projectPath: "/home/operator/work/orquestra",
    createdAt: "2026-10-18T09:55:00Z",
    updatedAt: "2026-10-18T10:05:00Z",
    sizeBytes: 120_832,
    turns: CODEX_TURNS,
  },
  {
    harness: "claude",
    id: "fixture-claude",
    title: "Transcript pipeline review",
    projectPath: "/home/operator/work/orquestra",
    createdAt: "2026-10-18T11:12:00Z",
    updatedAt: "2026-10-18T11:20:00Z",
    sizeBytes: 76_544,
    pinned: true,
    turns: CLAUDE_TURNS,
  },
  {
    harness: "freebuff",
    id: "fixture-freebuff",
    title: "Desktop parts JSON cleanup",
    projectPath: "/home/operator/work/freebuff-desktop",
    createdAt: "2026-10-18T07:52:00Z",
    updatedAt: "2026-10-18T07:58:00Z",
    sizeBytes: 54_016,
    turns: FREEBUFF_TURNS,
  },
  {
    harness: "copilot",
    id: "fixture-copilot",
    title: "Copilot turns table audit",
    projectPath: "/home/operator/work/billing-api",
    createdAt: "2026-10-18T06:28:00Z",
    updatedAt: "2026-10-18T06:33:00Z",
    sizeBytes: 18_944,
    turns: COPILOT_TURNS,
  },
  {
    harness: "hermes",
    id: "fixture-hermes",
    title: "Hermes migration triage",
    projectPath: "/home/operator/work/billing-api",
    createdAt: "2026-10-18T05:41:00Z",
    updatedAt: "2026-10-18T05:47:00Z",
    sizeBytes: 22_528,
    turns: HERMES_TURNS,
  },
  {
    harness: "opencode",
    id: "fixture-opencode",
    title: "OpenCode text parts join",
    projectPath: "/home/operator/work/edge-worker",
    createdAt: "2026-10-18T04:12:00Z",
    updatedAt: "2026-10-18T04:20:00Z",
    sizeBytes: 33_792,
    turns: OPENCODE_TURNS,
  },
  {
    harness: "kilo",
    id: "fixture-kilo",
    title: "Kilo code-part extraction",
    projectPath: "/home/operator/work/edge-worker",
    createdAt: "2026-10-18T03:08:00Z",
    updatedAt: "2026-10-18T03:15:00Z",
    sizeBytes: 27_648,
    turns: KILO_TURNS,
  },
];

/** The nine fixture sessions, pinned state included. */
export const FIXTURE_SESSIONS: UnifiedSession[] = FIXTURES.map((fixture) => ({
  id: fixture.id,
  harness: fixture.harness,
  title: fixture.title,
  projectPath: fixture.projectPath,
  updatedAt: fixture.updatedAt,
  createdAt: fixture.createdAt,
  turnsCount: fixture.turns.length,
  sizeBytes: fixture.sizeBytes,
  isPinned: fixture.pinned ?? false,
  pinLabel: null,
}));

/** Turns for one fixture session, or `undefined` when it is not a fixture. */
export function fixtureTranscript(
  harness: string,
  sessionId: string,
): TranscriptTurn[] | undefined {
  const fixture = FIXTURES.find(
    (entry) => entry.harness === harness && entry.id === sessionId,
  );
  return fixture ? fixture.turns.map((turn) => ({ ...turn })) : undefined;
}
