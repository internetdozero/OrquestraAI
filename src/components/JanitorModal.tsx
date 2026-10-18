import { useEffect } from "react";
import {
  Activity,
  AlertTriangle,
  Cpu,
  Loader2,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import type { SystemHealth } from "../lib/janitor";
import { cn } from "../lib/utils";

interface JanitorModalProps {
  health: SystemHealth | null;
  /** Operator's RAM threshold from settings; the RAM gauge turns amber past it. */
  ramWarning: number;
  loading: boolean;
  sweeping: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onSweep: () => void;
}

function Gauge({
  label,
  used,
  total,
  percent,
  warning,
}: {
  label: string;
  used: number;
  total: number;
  percent: number;
  warning: number;
}) {
  const { t } = useI18n();
  const value = Math.max(0, Math.min(100, percent));
  const elevated = percent > warning;
  return (
    <div className="rounded-xl border border-white/[0.06] bg-white/[0.02] p-3.5">
      <div className="flex items-center justify-between gap-3">
        <span className="text-[11px] font-medium text-zinc-300">{label}</span>
        <span className={cn("font-mono text-xs", elevated ? "text-amber-300" : "text-emerald-300")}>
          {Math.round(percent)}%
        </span>
      </div>
      <div
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(value)}
        className="mt-2 h-1.5 overflow-hidden rounded-full bg-white/[0.07]"
      >
        <div
          className={cn("h-full rounded-full transition-[width]", elevated ? "bg-amber-400" : "bg-gradient-to-r from-emerald-500 to-cyan-400")}
          style={{ width: `${value}%` }}
        />
      </div>
      <p className="mt-2 font-mono text-[10px] text-zinc-500">
        {used.toLocaleString()} / {total.toLocaleString()} {t.janitor.mb}
      </p>
    </div>
  );
}

export function JanitorModal({
  health,
  ramWarning,
  loading,
  sweeping,
  onClose,
  onRefresh,
  onSweep,
}: JanitorModalProps) {
  const { t } = useI18n();

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !sweeping) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, sweeping]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !sweeping) onClose();
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="janitor-title"
        className="flex max-h-[86vh] w-full max-w-xl flex-col overflow-hidden rounded-2xl border border-white/10 bg-[#0b0b0e] shadow-[0_30px_80px_-20px_rgba(0,0,0,0.9)]"
      >
        <header className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div className="flex items-start gap-3">
            <div className="grid size-9 shrink-0 place-items-center rounded-lg border border-cyan-500/25 bg-cyan-500/10 text-cyan-300">
              <Activity className="size-4" />
            </div>
            <div>
              <h2 id="janitor-title" className="text-sm font-semibold text-zinc-100">
                {t.janitor.title}
              </h2>
              <p className="mt-0.5 text-[11px] text-zinc-500">{t.janitor.subtitle}</p>
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={onRefresh}
              disabled={loading}
              aria-label={t.janitor.refresh}
              className="rounded-md p-1.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200 disabled:opacity-40"
            >
              <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
            </button>
            <button
              type="button"
              onClick={onClose}
              disabled={sweeping}
              aria-label={t.common.close}
              className="rounded-md p-1.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200 disabled:opacity-40"
            >
              <X className="size-4" />
            </button>
          </div>
        </header>

        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-5 py-4">
          {health ? (
            <>
              {health.alert && (
                <div role="status" className="flex items-center gap-2 rounded-lg border border-amber-500/25 bg-amber-500/[0.07] px-3 py-2 text-[11px] font-medium text-amber-200">
                  <AlertTriangle className="size-3.5 shrink-0 text-amber-400" />
                  {t.janitor.alert}
                </div>
              )}

              <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-2">
                <Gauge label={t.janitor.ram} used={health.ram.usedMb} total={health.ram.totalMb} percent={health.ram.usedPercent} warning={ramWarning} />
                <Gauge label={t.janitor.tmp} used={health.tmp.usedMb} total={health.tmp.totalMb} percent={health.tmp.usedPercent} warning={80} />
                <Gauge label={t.janitor.swap} used={health.swap.usedMb} total={health.swap.totalMb} percent={health.swap.usedPercent} warning={Number.POSITIVE_INFINITY} />
                <div className="flex items-center gap-3 rounded-xl border border-white/[0.06] bg-white/[0.02] p-3.5">
                  <Cpu className="size-4 text-cyan-300" />
                  <div>
                    <p className="text-[11px] font-medium text-zinc-300">{t.janitor.cpu}</p>
                    <p className="mt-1 font-mono text-xs text-zinc-200">
                      {health.cpu.cores} {t.janitor.cores} · {health.cpu.loadPercent.toFixed(0)}%
                    </p>
                  </div>
                </div>
              </div>

              <div>
                <div className="mb-2 flex items-center justify-between">
                  <h3 className="text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">{t.janitor.topProcesses}</h3>
                  <span className="text-[10px] text-zinc-600">{t.janitor.tmpPath(health.tmp.path, health.tmp.isTmpfs)}</span>
                </div>
                {health.topProcesses.length > 0 ? (
                  <ul className="divide-y divide-white/[0.05] rounded-lg border border-white/[0.06] bg-white/[0.015] px-3">
                    {health.topProcesses.map((process) => (
                      <li key={process.pid} className="flex items-center gap-3 py-2 text-[11px]">
                        <span className="w-12 shrink-0 font-mono text-zinc-600">{process.pid}</span>
                        <span className="min-w-0 flex-1 truncate text-zinc-300">{process.name}</span>
                        <span className="font-mono text-zinc-500">{process.rssMb} {t.janitor.mb}</span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="rounded-lg border border-white/[0.06] px-3 py-4 text-center text-[11px] text-zinc-500">{t.janitor.noProcesses}</p>
                )}
              </div>
            </>
          ) : (
            <div className="flex min-h-44 items-center justify-center gap-2 text-[11px] text-zinc-500">
              {loading ? <Loader2 className="size-4 animate-spin" /> : <AlertTriangle className="size-4" />}
              {loading ? t.janitor.loading : t.janitor.unavailable}
            </div>
          )}
        </div>

        <footer className="flex items-center justify-between gap-3 border-t border-white/[0.06] bg-white/[0.015] px-5 py-3">
          <p className="text-[10px] text-zinc-600">{t.janitor.sweepHint}</p>
          <button
            type="button"
            onClick={onSweep}
            disabled={sweeping || !health}
            className="flex shrink-0 items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/15 px-3 py-1.5 text-[11px] font-semibold text-emerald-200 transition hover:border-emerald-400/50 hover:bg-emerald-500/25 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {sweeping ? <Loader2 className="size-3.5 animate-spin" /> : <Trash2 className="size-3.5" />}
            {sweeping ? t.janitor.sweeping : t.janitor.sweep}
          </button>
        </footer>
      </section>
    </div>
  );
}
