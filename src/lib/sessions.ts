import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./tauri";

/** Canonical harness ids, in the order the core lists them. */
export type SessionHarness =
  | "antigravity"
  | "codex"
  | "claude"
  | "freebuff"
  | "opencode"
  | "kilo"
  | "copilot"
  | "hermes"
  | "grok";

/** Mirrors the Rust `UnifiedSession` struct: one conversation on disk. */
export interface UnifiedSession {
  id: string;
  /** Canonical harness id this conversation belongs to. */
  harness: string;
  /** Summary or first prompt, already trimmed by the core. */
  title: string;
  /** Workspace the harness recorded, when it recorded one. */
  projectPath: string | null;
  /** ISO 8601 UTC. */
  updatedAt: string;
  createdAt: string | null;
  turnsCount: number;
  /** Estimated bytes on disk: session files plus content rows. */
  sizeBytes: number;
  /** Protected from every purge; stored in ~/.orquestra, never in a harness. */
  isPinned: boolean;
  pinLabel: string | null;
}

/** Which sessions a purge may touch. Mirrors the Rust `PurgeFilter` struct. */
export interface PurgeFilter {
  /** Restrict to one canonical harness id. */
  harness?: string | null;
  /** Only sessions untouched for at least this many days. */
  olderThanDays?: number | null;
  /** Surgical deletes: exact session ids. */
  sessionIds?: string[] | null;
  /** Never delete a pinned session while this is true (the default). */
  ignorePinned?: boolean;
}

/** What a purge *would* do, computed without touching anything on disk. */
export interface PurgeEstimate {
  sessionsCount: number;
  reclaimedBytes: number;
  /** Canonical harness ids the purge would touch. */
  affectedHarnesses: string[];
  /** Protected by a pin and therefore left alone. */
  skippedPinnedCount: number;
  /** Held open by a running agent and therefore left alone. */
  skippedActiveCount: number;
}

/** Mirrors the Rust `PurgeReport` struct. */
export interface PurgeReport {
  deletedCount: number;
  /** Bytes of session files and folders removed; `VACUUM` savings are not
   * included, because they happen after the rows are gone. */
  reclaimedBytes: number;
  failedCount: number;
  errors: string[];
}

/** Mirrors the Rust `RetentionPolicy` struct. */
export interface RetentionPolicy {
  defaultTtlDays: number;
  /** Per-harness TTL overrides, keyed by canonical harness id. */
  harnessOverrides: Record<string, number>;
  /** Opt-in: nothing is deleted on a schedule until the operator asks. */
  autoPurgeEnabled: boolean;
  autoPurgeIntervalHours: number;
  lastRun: string | null;
}

/** Every harness conversation, newest first, with pin state applied. */
export function getUnifiedSessions(): Promise<UnifiedSession[]> {
  return invoke<UnifiedSession[]>("get_unified_sessions");
}

/** One conversation turn. Mirrors the Rust `TranscriptTurn` struct. */
export interface TranscriptTurn {
  role: "user" | "assistant" | "system";
  content: string;
  timestamp?: string;
}

/** Conversation turns for the detail drawer's feed; empty outside Tauri or
 * for harnesses without a transcript reader yet. */
export async function getSessionTranscript(
  harness: string,
  sessionId: string,
): Promise<TranscriptTurn[]> {
  if (!isTauri()) return [];
  return invoke<TranscriptTurn[]>("get_session_transcript", {
    harness,
    sessionId,
  });
}

/** Warm index of the unified catalog, filled once per boot by the background preload. */
let warmedSessions: UnifiedSession[] | null = null;

/**
 * Best-effort boot warm-up: index the catalog so the Sessions view paints
 * from memory instead of walking ~1.2k session files on first open.
 */
export function ensureUnifiedSessions(): Promise<UnifiedSession[]> {
  if (warmedSessions) return Promise.resolve(warmedSessions);
  return getUnifiedSessions().then((list) => {
    warmedSessions = list;
    return list;
  });
}

/** The warmed catalog, when the background preload has already finished. */
export function peekWarmedSessions(): UnifiedSession[] | null {
  return warmedSessions;
}

/** Pin or unpin one conversation; resolves to the state after the toggle. */
export function togglePinSession(
  harness: string,
  sessionId: string,
  label?: string | null,
): Promise<boolean> {
  return invoke<boolean>("toggle_pin_session", {
    harness,
    sessionId,
    label: label ?? null,
  });
}

export function getRetentionPolicy(): Promise<RetentionPolicy> {
  return invoke<RetentionPolicy>("get_retention_policy");
}

/** Replace the retention rules; the pin list is preserved by the core. */
export function saveRetentionPolicy(policy: RetentionPolicy): Promise<void> {
  return invoke<void>("save_retention_policy", { policy });
}

/** Dry run: what `purgeSessions` would delete for this filter. */
export function estimatePurge(filter: PurgeFilter): Promise<PurgeEstimate> {
  return invoke<PurgeEstimate>("estimate_purge", { filter });
}

/** Delete every session the filter matches, keeping pinned and open ones. */
export function purgeSessions(filter: PurgeFilter): Promise<PurgeReport> {
  return invoke<PurgeReport>("purge_sessions", { filter });
}
