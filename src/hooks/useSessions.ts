import { useCallback, useEffect, useMemo, useState } from "react";
import {
  estimatePurge,
  getRetentionPolicy,
  getUnifiedSessions,
  peekWarmedSessions,
  purgeSessions,
  saveRetentionPolicy,
  togglePinSession,
  type PurgeEstimate,
  type PurgeFilter,
  type RetentionPolicy,
  type UnifiedSession,
} from "../lib/sessions";
import { formatMb } from "../lib/sessionFormat";
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

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

/** Last catalog, kept outside the hook so remounting the view (tab switch)
 * paints instantly and revalidates in the background instead of walking
 * ~1.2k session files with a skeleton on screen. */
let cachedSessions: UnifiedSession[] | null = null;

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
  const [sessions, setSessions] = useState<UnifiedSession[]>(() => cachedSessions ?? peekWarmedSessions() ?? []);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<SessionFilterState>(INITIAL_FILTER);
  const [policy, setPolicy] = useState<RetentionPolicy | null>(null);
  const [pendingIds, setPendingIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [purgeFilter, setPurgeFilter] = useState<PurgeFilter | null>(null);
  const [estimate, setEstimate] = useState<PurgeEstimate | null>(null);
  const [estimating, setEstimating] = useState(false);
  const [purging, setPurging] = useState(false);
  const [retentionOpen, setRetentionOpen] = useState(false);

  const refreshSessions = useCallback(async () => {
    if (!isTauri) return;
    // Warm cache: paint instantly, revalidate silently. Cold start keeps
    // the loading banner so first paint never flashes an empty catalog.
    const cold = cachedSessions === null && peekWarmedSessions() === null;
    if (cold) setLoading(true);
    setError(null);
    try {
      const [list, retention] = await Promise.all([
        getUnifiedSessions(),
        getRetentionPolicy(),
      ]);
      cachedSessions = list;
      setSessions(list);
      setPolicy(retention);
    } catch (cause) {
      const message = t.sessions.loadFailed(
        toMessage(cause, t.common.unexpected),
      );
      setError(message);
      onError(message);
    } finally {
      setLoading(false);
    }
  }, [isTauri, onError, t]);

  useEffect(() => {
    void refreshSessions();
  }, [refreshSessions]);

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
      if (!isTauri) return;
      const key = `${harness}:${sessionId}`;
      setPendingIds((ids) => new Set(ids).add(key));
      try {
        const pinned = await togglePinSession(harness, sessionId);
        // Keep the module cache coherent so a tab switch never shows a
        // stale pin until the next background revalidation lands.
        setSessions((rows) => {
          const next = rows.map((s) =>
            s.harness === harness && s.id === sessionId ? { ...s, isPinned: pinned } : s,
          );
          cachedSessions = next;
          return next;
        });
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
    [isTauri, onError, onToast, t],
  );

  const openPurgeModal = useCallback(
    async (next: PurgeFilter) => {
      if (!isTauri) return;
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
    [isTauri, onError, t],
  );

  const closePurgeModal = useCallback(() => {
    if (purging) return;
    setPurgeFilter(null);
    setEstimate(null);
  }, [purging]);

  const executePurge = useCallback(async () => {
    if (!isTauri || !purgeFilter || purging) return;
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
  }, [isTauri, purgeFilter, purging, notifyOnPurge, onToast, onError, refreshSessions, t]);

  const deleteSingleSession = useCallback(
    async (harness: string, sessionId: string) => {
      if (!isTauri) return;
      const before = sessions.find(
        (s) => s.harness === harness && s.id === sessionId,
      );
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
        void before;
        onError(t.sessions.purgeFailed(toMessage(cause, t.common.unexpected)));
      }
    },
    [isTauri, notifyOnPurge, onError, onToast, refreshSessions, sessions, t],
  );

  const updatePolicy = useCallback(
    async (next: RetentionPolicy) => {
      if (!isTauri) return;
      try {
        await saveRetentionPolicy(next);
        setPolicy(next);
        setRetentionOpen(false);
        onToast(t.sessions.saved);
      } catch (cause) {
        onError(t.sessions.saveFailed(toMessage(cause, t.common.unexpected)));
      }
    },
    [isTauri, onError, onToast, t],
  );

  return {
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
