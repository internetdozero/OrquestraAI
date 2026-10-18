import { createPortal } from "react-dom";
import { ShieldCheck, TriangleAlert } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { PurgeEstimate, PurgeFilter } from "../../lib/sessions";
import { formatBytes } from "../../lib/sessionFormat";
import { scopeLabel } from "../../lib/harnesses";
import { cn } from "../../lib/utils";

interface PurgeConfirmModalProps {
  filter: PurgeFilter;
  estimate: PurgeEstimate | null;
  estimating: boolean;
  purging: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

/** Dry-run preview before any destructive purge. Never renders without a filter. */
export function PurgeConfirmModal({
  filter,
  estimate,
  estimating,
  purging,
  onCancel,
  onConfirm,
}: PurgeConfirmModalProps) {
  const { t } = useI18n();
  const s = t.sessions;
  const scope =
    filter.harness != null
      ? scopeLabel(filter.harness)
      : s.harnessAll;
  const age =
    filter.olderThanDays != null ? ` · > ${filter.olderThanDays}d` : "";

  if (typeof document === "undefined") return null;

  // Same stacking-context fix as the retention drawer: this modal also
  // renders inside <main>, beneath the header/footer layers.
  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={s.purgeModalTitle}
      className="fixed inset-0 z-[100] grid place-items-center bg-black/60 p-4 backdrop-blur-sm"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-md rounded-2xl border border-white/10 bg-[#101014] p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2.5">
          <div className="grid size-9 place-items-center rounded-xl border border-rose-500/30 bg-rose-500/10">
            <TriangleAlert className="size-4 text-rose-300" />
          </div>
          <div>
            <h2 className="text-sm font-semibold text-zinc-100">{s.purgeModalTitle}</h2>
            <p className="font-mono text-[10.5px] text-zinc-500">
              {scope}{age}
            </p>
          </div>
        </div>

        {estimating || !estimate ? (
          <p className="mt-4 animate-pulse text-[12.5px] text-zinc-400">{s.estimating}</p>
        ) : (
          <div className="mt-4 space-y-3">
            <p className="text-[12.5px] leading-relaxed text-zinc-300">
              {s.purgeModalLead(estimate.sessionsCount)}
            </p>
            <div className="rounded-xl border border-white/[0.07] bg-white/[0.02] p-3">
              <p className="text-[10.5px] tracking-wider text-zinc-500 uppercase">{s.reclaimed}</p>
              <p className="mt-0.5 font-mono text-2xl font-semibold text-rose-200">
                {formatBytes(estimate.reclaimedBytes)}
              </p>
              <p className="mt-1 text-[11px] text-zinc-500">
                {s.affected}: {estimate.affectedHarnesses.map(scopeLabel).join(", ") || "—"}
              </p>
            </div>
            <p className="flex items-start gap-1.5 text-[11.5px] leading-relaxed text-emerald-200/90">
              <ShieldCheck className="mt-px size-3.5 shrink-0" />
              {s.pinnedSafe(estimate.skippedPinnedCount)}
              {estimate.skippedActiveCount > 0 && (
                <span className="block">{s.activeSafe(estimate.skippedActiveCount)}</span>
              )}
            </p>
          </div>
        )}

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            disabled={purging}
            className="rounded-lg border border-white/10 bg-white/[0.03] px-3.5 py-2 text-[12px] font-medium text-zinc-300 transition hover:border-white/20 disabled:opacity-50"
          >
            {s.cancel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={estimating || purging || !estimate || estimate.sessionsCount === 0}
            className={cn(
              "rounded-lg bg-rose-600 px-3.5 py-2 text-[12px] font-semibold text-white transition",
              "hover:bg-rose-500 disabled:cursor-not-allowed disabled:opacity-50",
            )}
          >
            {purging ? s.estimating : s.confirmPurge}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
