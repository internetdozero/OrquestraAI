import { useEffect, useState } from "react";
import { History, SearchX } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { UnifiedSession } from "../../lib/sessions";
import { StatePanel } from "../ui";
import { SessionRow } from "./SessionRow";

const PAGE_SIZE = 25;

interface SessionsTableProps {
  sessions: UnifiedSession[];
  loading: boolean;
  /** True when a real core or the dev fixture store can serve sessions. */
  enabled: boolean;
  pendingIds: ReadonlySet<string>;
  onTogglePin: (harness: string, sessionId: string) => void;
  onDelete: (harness: string, sessionId: string) => void;
  onInspect: (session: UnifiedSession) => void;
}

/**
 * Incrementally rendered card list: only the first page hits the DOM, the
 * rest loads 25 at a time. A new filtered array (any filter change)
 * resets back to the first page.
 */
export function SessionsTable({
  sessions,
  loading,
  enabled,
  pendingIds,
  onTogglePin,
  onDelete,
  onInspect,
}: SessionsTableProps) {
  const { t } = useI18n();
  const s = t.sessions;
  const [pageSize, setPageSize] = useState(PAGE_SIZE);

  useEffect(() => {
    setPageSize(PAGE_SIZE);
  }, [sessions]);

  if (!enabled) {
    return (
      <StatePanel icon={History} title={s.empty} description={s.unavailable} />
    );
  }

  if (loading && sessions.length === 0) {
    return (
      <div
        className="rounded-xl border border-white/[0.06] bg-zinc-900/30 px-5 py-8 text-center"
        aria-busy="true"
      >
        <p className="text-[12.5px] text-zinc-300">{s.loadingHistory}</p>
        <div className="mx-auto mt-4 max-w-md space-y-2">
          {Array.from({ length: 4 }, (_, i) => (
            <div
              key={i}
              className="h-9 animate-pulse rounded-lg border border-white/[0.04] bg-zinc-800/40"
            />
          ))}
        </div>
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <StatePanel icon={SearchX} title={s.empty} description={s.noMatch} />
    );
  }

  const shown = sessions.slice(0, pageSize);
  const remaining = sessions.length - shown.length;

  return (
    <div className="space-y-2" role="list" aria-label={s.title}>
      {shown.map((session) => (
        <div key={`${session.harness}:${session.id}`} role="listitem">
          <SessionRow
            session={session}
            busy={pendingIds.has(`${session.harness}:${session.id}`)}
            onTogglePin={() => onTogglePin(session.harness, session.id)}
            onDelete={() => onDelete(session.harness, session.id)}
            onInspect={() => onInspect(session)}
          />
        </div>
      ))}
      {remaining > 0 ? (
        <button
          type="button"
          onClick={() => setPageSize((count) => count + PAGE_SIZE)}
          className="w-full rounded-lg border border-white/10 bg-white/[0.02] py-2.5 text-xs text-zinc-400 transition hover:bg-white/[0.04] hover:text-zinc-200"
        >
          {s.loadMore(remaining)}
        </button>
      ) : (
        sessions.length > PAGE_SIZE && (
          <p className="pt-1 text-center font-mono text-[10.5px] text-zinc-600">
            {s.showingOf(sessions.length, sessions.length)}
          </p>
        )
      )}
    </div>
  );
}
