import { useCallback, useEffect, useMemo, useState, type SetStateAction } from "react";
import {
  SESSIONS_CATALOG_KEY,
  SESSIONS_CATALOG_TTL_MS,
  SESSIONS_POLICY_KEY,
  SESSIONS_POLICY_TTL_MS,
  estimatePurge,
  getUnifiedSessions,
  loadRetentionPolicy,
  purgeSessions,
  saveRetentionPolicyCached,
  togglePinSession,
  type PurgeEstimate,
  type PurgeFilter,
  type RetentionPolicy,
  type UnifiedSession,
} from "../lib/sessions";
import { fixturesEnabled } from "../lib/fixtures";
import { formatMb } from "../lib/sessionFormat";
import { useCachedResource } from "./useCachedResource";
import { useI18n } from "./useI18n";

export type SessionSort = "newest" | "oldest" | "largest";

export interface SessionFilterState {
  query: string;
  harness: string;
  onlyPinned: boolean;
  purgeCandidates: boolean;
  sort: SessionSort;
}

const INITIAL_FILTER: SessionFilterState = {
  query: "",
  harness: "all",
  onlyPinned: false,
  purgeCandidates: false,
  sort: "newest",
};

/**
 * The shell unmounts the whole view on a tab switch, so the filter would reset
 * on every visit. Keeping it beside the hook makes "back to Sessions" land
 * where the operator left off.
 */
let lastFilter: SessionFilterState = INITIAL_FILTER;

/** Stable identity so the resource state is not reset between renders. */
const EMPTY_SESSIONS: UnifiedSession[] = [];

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

function isOld(s: UnifiedSession, days: number): boolean {
  const then = Date.parse(s.updatedAt);
  if (Number.isNaN(then)) return false;
  return Date.now() - then >= days * 86_400_000;
}

/**
 * All Sessions state in one place: catalog, filters, retention policy and
 * the dry-run-backed purge flow. Purge modals always open through
 * `openPurgeModal`, which fetches `estimate_purge` first so the destructive
 * action is previewed before it runs.
 *
 * The catalog and the retention policy are separate cache entries with their
 * own TTLs (`lib/sessions`): opening the tab again within the TTL paints from
 * memory and never repeats the disk walk, while a stale catalog is repainted in
 * place and revalidated in the background instead of blanking the table.
 */
export function useSessions(options: {
  isTauri: boolean;
  /** Gates purge-completion toasts; errors always surface. */
  notifyOnPurge: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
}) {
  const { isTauri, notifyOnPurge, onToast, onError } = options;
  const { t } = useI18n();
  // Dev fixture mode keeps the view usable in a plain browser preview; every
  // IPC entry point has a matching in-memory implementation in `lib/fixtures`.
  const [fixtureMode] = useState(fixturesEnabled);
  const enabled = isTauri || fixtureMode;

  const catalog = useCachedResource<UnifiedSession[]>({
    key: SESSIONS_CATALOG_KEY,
    ttlMs: SESSIONS_CATALOG_TTL_MS,
    enabled,
    fallback: EMPTY_SESSIONS,
    load: getUnifiedSessions,
    perfView: "sessions",
  });

  const policyResource = useCachedResource<RetentionPolicy | null>({
    key: SESSIONS_POLICY_KEY,
    ttlMs: SESSIONS_POLICY_TTL_MS,
    enabled,
    fallback: null,
    load: loadRetentionPolicy,
  });

  const sessions = catalog.data;
  const policy = policyResource.data;
  const loading = catalog.loading;

  const [error, setError] = useState<string | null>(null);
  const [filter, setFilterState] = useState<SessionFilterState>(lastFilter);
  const setFilter = useCallback((next: SetStateAction<SessionFilterState>) => {
    setFilterState((previous) => {
      const value = typeof next === "function" ? next(previous) : next;
      lastFilter = value;
      return value;
    });
  }, []);
  const [pendingIds, setPendingIds] = useState<ReadonlySet<string>>(() => new Set());
  const [purgeFilter, setPurgeFilter] = useState<PurgeFilter | null>(null);
  const [estimate, setEstimate] = useState<PurgeEstimate | null>(null);
  const [estimating, setEstimating] = useState(false);
  const [purging, setPurging] = useState(false);
  const [retentionOpen, setRetentionOpen] = useState(false);

  // Destructured: the resource objects are re-created every render, so a
  // callback closing over them would never be stable.
  const { refresh: reloadCatalog, mutate: mutateCatalog } = catalog;
  const { refresh: reloadPolicy, mutate: mutatePolicy } = policyResource;
  const failure = catalog.error ?? policyResource.error;

  // One place turns either resource's failure into the operator-facing banner.
  useEffect(() => {
    if (!failure) return;
    const message = t.sessions.loadFailed(toMessage(failure, t.common.unexpected));
    setError(message);
    onError(message);
  }, [failure, onError, t]);

  /** Unconditional rescan, for the shell refresh button and after a deletion. */
  const refreshSessions = useCallback(async () => {
    if (!enabled) return;
    setError(null);
    await Promise.all([reloadCatalog(), reloadPolicy()]);
  }, [enabled, reloadCatalog, reloadPolicy]);

  const visible = useMemo(() => {
    const needle = filter.query.trim().toLowerCase();
    const rows = sessions.filter((s) => {
      if (filter.harness !== "all" && s.harness !== filter.harness) return false;
      if (filter.onlyPinned && !s.isPinned) return false;
      if (filter.purgeCandidates && (s.isPinned || !isOld(s, 7))) return false;
      if (!needle) return true;
      return [s.title, s.projectPath ?? "", s.harness, s.id].some((f) =>
        f.toLowerCase().includes(needle),
      );
    });
    return [...rows].sort((a, b) => {
      if (filter.sort === "largest") return b.sizeBytes - a.sizeBytes;
      const at = Date.parse(a.updatedAt);
      const bt = Date.parse(b.updatedAt);
      return filter.sort === "oldest" ? at - bt : bt - at;
    });
  }, [sessions, filter]);

  const metrics = useMemo(() => {
    const perHarness: Record<string, number> = {};
    let bytes = 0;
    let pinned = 0;
    for (const s of sessions) {
      bytes += s.sizeBytes;
      perHarness[s.harness] = (perHarness[s.harness] ?? 0) + 1;
      if (s.isPinned) pinned += 1;
    }
    return {
      total: sessions.length,
      bytes,
      pinned,
      perHarness,
    };
  }, [sessions]);

  const togglePin = useCallback(
    async (harness: string, sessionId: string) => {
      if (!enabled) return;
      const key = `${harness}:${sessionId}`;
      setPendingIds((ids) => new Set(ids).add(key));
      try {
        const pinned = await togglePinSession(harness, sessionId);
        // Optimistic: the cache and the table move together, so a tab switch
        // never flashes the previous pin state while the rescan is pending.
        mutateCatalog(
          sessions.map((s) =>
            s.harness === harness && s.id === sessionId ? { ...s, isPinned: pinned } : s,
          ),
        );
        onToast(pinned ? t.sessions.pinnedToast : t.sessions.unpinnedToast);
      } catch (cause) {
        onError(t.sessions.loadFailed(toMessage(cause, t.common.unexpected)));
      } finally {
        setPendingIds((ids) => {
          const next = new Set(ids);
          next.delete(key);
          return next;
        });
      }
    },
    [enabled, onError, onToast, sessions, mutateCatalog, t],
  );

  const openPurgeModal = useCallback(
    async (next: PurgeFilter) => {
      if (!enabled) return;
      setPurgeFilter(next);
      setEstimate(null);
      setEstimating(true);
      try {
        setEstimate(await estimatePurge(next));
      } catch (cause) {
        onError(t.sessions.purgeFailed(toMessage(cause, t.common.unexpected)));
        setPurgeFilter(null);
      } finally {
        setEstimating(false);
      }
    },
    [enabled, onError, t],
  );

  const closePurgeModal = useCallback(() => {
    if (purging) return;
    setPurgeFilter(null);
    setEstimate(null);
  }, [purging]);

  const executePurge = useCallback(async () => {
    if (!enabled || !purgeFilter || purging) return;
    setPurging(true);
    try {
      const report = await purgeSessions(purgeFilter);
      if (notifyOnPurge) {
        onToast(t.sessions.purged(report.deletedCount, formatMb(report.reclaimedBytes)));
      }
      setPurgeFilter(null);
      setEstimate(null);
      await refreshSessions();
    } catch (cause) {
      onError(t.sessions.purgeFailed(toMessage(cause, t.common.unexpected)));
    } finally {
      setPurging(false);
    }
  }, [enabled, purgeFilter, purging, notifyOnPurge, onToast, onError, refreshSessions, t]);

  const deleteSingleSession = useCallback(
    async (harness: string, sessionId: string) => {
      if (!enabled) return;
      try {
        const report = await purgeSessions({
          harness,
          sessionIds: [sessionId],
          ignorePinned: false,
        });
        if (notifyOnPurge) {
          onToast(t.sessions.deleted(formatMb(report.reclaimedBytes)));
        }
        await refreshSessions();
      } catch (cause) {
        onError(t.sessions.purgeFailed(toMessage(cause, t.common.unexpected)));
      }
    },
    [enabled, notifyOnPurge, onError, onToast, refreshSessions, t],
  );

  const updatePolicy = useCallback(
    async (next: RetentionPolicy) => {
      if (!enabled) return;
      try {
        await saveRetentionPolicyCached(next);
        mutatePolicy(next);
        setRetentionOpen(false);
        onToast(t.sessions.saved);
      } catch (cause) {
        onError(t.sessions.saveFailed(toMessage(cause, t.common.unexpected)));
      }
    },
    [enabled, onError, onToast, mutatePolicy, t],
  );

  return {
    enabled,
    fixtureMode,
    sessions: visible,
    totalCount: sessions.length,
    loading,
    error,
    filter,
    setFilter,
    metrics,
    policy,
    pendingIds,
    refreshSessions,
    togglePin,
    openPurgeModal,
    closePurgeModal,
    executePurge,
    deleteSingleSession,
    updatePolicy,
    purgeFilter,
    estimate,
    estimating,
    purging,
    retentionOpen,
    setRetentionOpen,
  };
}

export type SessionsState = ReturnType<typeof useSessions>;
