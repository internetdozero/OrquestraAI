import { useEffect, useRef, useState } from "react";
import { History } from "lucide-react";
import { useSessions } from "../../hooks/useSessions";
import { useI18n } from "../../hooks/useI18n";
import type { UnifiedSession } from "../../lib/sessions";
import { ErrorBanner } from "../ui";
import { ConfirmModal } from "../ConfirmModal";
import { SessionDetailDrawer } from "./SessionDetailDrawer";
import { SessionsHeader } from "./SessionsHeader";
import { SessionsToolbar } from "./SessionsToolbar";
import { SessionsTable } from "./SessionsTable";
import { PurgeConfirmModal } from "./PurgeConfirmModal";
import { RetentionDrawer } from "./RetentionDrawer";

interface SessionsViewProps {
  isTauri: boolean;
  /** Gates purge-completion toasts (settings → notifyOnPurge). */
  notifyOnPurge: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
  /** Shell refresh button bumps this to rescan the sessions. */
  reloadToken?: number;
}

/** First-class Sessions view: inspect, filter, pin, purge, configure retention. */
export function SessionsView({
  isTauri,
  notifyOnPurge,
  onToast,
  onError,
  reloadToken = 0,
}: SessionsViewProps) {
  const { t } = useI18n();
  const state = useSessions({ isTauri, notifyOnPurge, onToast, onError });
  const { refreshSessions } = state;

  // The shell asks for a rescan by bumping the token; skip the mount value.
  const lastReloadToken = useRef(reloadToken);
  useEffect(() => {
    if (reloadToken === lastReloadToken.current) return;
    lastReloadToken.current = reloadToken;
    void refreshSessions();
  }, [reloadToken, refreshSessions]);
  const s = t.sessions;
  const [inspecting, setInspecting] = useState<UnifiedSession | null>(null);
  const [pendingDelete, setPendingDelete] = useState<{
    harness: string;
    id: string;
    title: string;
  } | null>(null);
  // Re-resolve against the live list so pin toggles repaint the drawer.
  const liveInspecting =
    inspecting == null
      ? null
      : (state.sessions.find(
          (session) => session.harness === inspecting.harness && session.id === inspecting.id,
        ) ?? inspecting);

  return (
    <div className="mx-auto w-full max-w-[1200px] space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <div className="grid size-9 place-items-center rounded-xl border border-emerald-500/25 bg-emerald-500/10">
            <History className="size-4 text-emerald-300" />
          </div>
          <div>
            <h2 className="text-[15px] font-semibold tracking-tight text-zinc-100">{s.title}</h2>
            <p className="text-[11.5px] text-zinc-500">{s.subtitle}</p>
          </div>
        </div>
      </div>

      {state.error && (
        <ErrorBanner message={state.error} onDismiss={() => void state.refreshSessions()} />
      )}

      {state.fixtureMode && (
        <p
          role="status"
          className="rounded-lg border border-cyan-500/25 bg-cyan-500/[0.06] px-3 py-1.5 font-mono text-[10.5px] text-cyan-200/90"
        >
          {s.fixturesBanner}
        </p>
      )}

      <SessionsHeader
        total={state.metrics.total}
        bytes={state.metrics.bytes}
        pinned={state.metrics.pinned}
        autoEnabled={state.policy?.autoPurgeEnabled ?? false}
        intervalHours={state.policy?.autoPurgeIntervalHours ?? 24}
        purging={state.purging || state.estimating}
        loading={state.loading}
        onQuickPurge={(days) =>
          void state.openPurgeModal({
            olderThanDays: days,
            ignorePinned: true,
          })
        }
        onOpenRetention={() => state.setRetentionOpen(true)}
      />

      <SessionsToolbar
        filter={state.filter}
        onChange={state.setFilter}
        visibleCount={state.sessions.length}
      />

      <SessionsTable
        sessions={state.sessions}
        loading={state.loading}
        enabled={state.enabled}
        pendingIds={state.pendingIds}
        onTogglePin={(h, id) => void state.togglePin(h, id)}
        onDelete={(h, id) => {
          const target = state.sessions.find(
            (session) => session.harness === h && session.id === id,
          );
          setPendingDelete({ harness: h, id, title: target?.title ?? id });
        }}
        onInspect={setInspecting}
      />

      {pendingDelete && (
        <ConfirmModal
          title={s.deleteTitle}
          message={s.deleteConfirm(pendingDelete.title || pendingDelete.id)}
          confirmLabel={t.common.delete}
          cancelLabel={s.cancel}
          tone="danger"
          onCancel={() => setPendingDelete(null)}
          onConfirm={() => {
            void state.deleteSingleSession(pendingDelete.harness, pendingDelete.id);
            setPendingDelete(null);
            setInspecting(null);
          }}
        />
      )}

      {state.purgeFilter && (
        <PurgeConfirmModal
          filter={state.purgeFilter}
          estimate={state.estimate}
          estimating={state.estimating}
          purging={state.purging}
          onCancel={state.closePurgeModal}
          onConfirm={() => void state.executePurge()}
        />
      )}

      {liveInspecting && (
        <SessionDetailDrawer
          session={liveInspecting}
          onClose={() => setInspecting(null)}
          onTogglePin={() => void state.togglePin(liveInspecting.harness, liveInspecting.id)}
          onDelete={() =>
            setPendingDelete({
              harness: liveInspecting.harness,
              id: liveInspecting.id,
              title: liveInspecting.title || liveInspecting.id,
            })
          }
          onToast={onToast}
        />
      )}

      {state.retentionOpen && (
        <RetentionDrawer
          policy={state.policy}
          onClose={() => state.setRetentionOpen(false)}
          onSave={(policy) => void state.updatePolicy(policy)}
        />
      )}
    </div>
  );
}
