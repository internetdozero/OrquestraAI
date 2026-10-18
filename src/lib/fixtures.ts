import { FIXTURE_SESSIONS, fixtureTranscript } from "../mocks/transcriptFixtures";
import type {
  PurgeEstimate,
  PurgeFilter,
  PurgeReport,
  RetentionPolicy,
  TranscriptTurn,
  UnifiedSession,
} from "./sessions";

/**
 * Developer fixture mode: makes the Sessions view and the transcript drawer
 * work in a plain Vite browser preview, with one mock conversation per
 * harness. Never active in a production build.
 *
 * Enable with `?fixtures=1` (remembered in localStorage) and disable with
 * `?fixtures=0`.
 */
const STORAGE_KEY = "orquestra.fixtureMode";

export function fixturesEnabled(): boolean {
  if (!import.meta.env.DEV || typeof window === "undefined") return false;
  // Never shadow real session data inside the desktop app, even in dev.
  if ("__TAURI_INTERNALS__" in window) return false;
  const flag = new URLSearchParams(window.location.search).get("fixtures");
  try {
    if (flag === "1") {
      window.localStorage.setItem(STORAGE_KEY, "1");
      return true;
    }
    if (flag === "0") {
      window.localStorage.removeItem(STORAGE_KEY);
      return false;
    }
    return window.localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return flag === "1";
  }
}

/** Mutable copies so pin/delete/purge exercise the real UI paths. */
const sessions: UnifiedSession[] = FIXTURE_SESSIONS.map((session) => ({ ...session }));

const DEFAULT_RETENTION: RetentionPolicy = {
  defaultTtlDays: 15,
  harnessOverrides: {},
  autoPurgeEnabled: false,
  autoPurgeIntervalHours: 24,
  lastRun: null,
};

let retention: RetentionPolicy = { ...DEFAULT_RETENTION };

export function fixtureSessions(): UnifiedSession[] {
  return sessions.map((session) => ({ ...session }));
}

export function fixtureSessionTranscript(
  harness: string,
  sessionId: string,
): TranscriptTurn[] | undefined {
  return fixtureTranscript(harness, sessionId);
}

export function fixtureRetention(): RetentionPolicy {
  return { ...retention, harnessOverrides: { ...retention.harnessOverrides } };
}

export function fixtureSaveRetention(policy: RetentionPolicy): void {
  retention = { ...policy, harnessOverrides: { ...policy.harnessOverrides } };
}

/** Flip one fixture's pin and report the state after the toggle. */
export function fixtureTogglePin(harness: string, sessionId: string): boolean {
  const session = sessions.find(
    (entry) => entry.harness === harness && entry.id === sessionId,
  );
  if (!session) return false;
  session.isPinned = !session.isPinned;
  if (!session.isPinned) session.pinLabel = null;
  return session.isPinned;
}

function matcher(filter: PurgeFilter): (session: UnifiedSession) => boolean {
  return (session) => {
    if (filter.harness && session.harness !== filter.harness) return false;
    if (filter.sessionIds && !filter.sessionIds.includes(session.id)) return false;
    if (filter.olderThanDays != null) {
      const then = Date.parse(session.updatedAt);
      if (Number.isNaN(then)) return false;
      const age = Date.now() - then;
      if (age < filter.olderThanDays * 86_400_000) return false;
    }
    return true;
  };
}

export function fixtureEstimate(filter: PurgeFilter): PurgeEstimate {
  const matched = sessions.filter(matcher(filter));
  const ignorePinned = filter.ignorePinned ?? true;
  const targets = matched.filter((session) => !(ignorePinned && session.isPinned));
  const harnesses = [...new Set(targets.map((session) => session.harness))].sort();

  return {
    sessionsCount: targets.length,
    reclaimedBytes: targets.reduce((total, session) => total + session.sizeBytes, 0),
    affectedHarnesses: harnesses,
    skippedPinnedCount: matched.length - targets.length,
    skippedActiveCount: 0,
  };
}

export function fixturePurge(filter: PurgeFilter): PurgeReport {
  const ignorePinned = filter.ignorePinned ?? true;
  const matches = matcher(filter);
  let deletedCount = 0;
  let reclaimedBytes = 0;

  for (let index = sessions.length - 1; index >= 0; index -= 1) {
    const session = sessions[index];
    if (!matches(session) || (ignorePinned && session.isPinned)) continue;
    deletedCount += 1;
    reclaimedBytes += session.sizeBytes;
    sessions.splice(index, 1);
  }

  return { deletedCount, reclaimedBytes, failedCount: 0, errors: [] };
}
