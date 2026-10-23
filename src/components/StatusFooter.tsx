import { Activity, AlertTriangle, FolderTree, Link2, Loader2, PlugZap, Radio } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import type { HarnessStatus } from "../lib/bridge";
import type { CoreStatus } from "../lib/tauri";
import type { SystemHealth } from "../lib/janitor";
import { cn } from "../lib/utils";

interface StatusFooterProps {
  status: CoreStatus | null;
  isTauri: boolean;
  bridgeStatuses: HarnessStatus[];
  bridgeDeploying: boolean;
  onManageBridge: () => void;
  janitorHealth: SystemHealth | null;
  /** Operator's RAM threshold: the chip turns amber past it, not just on the core's fixed alert. */
  ramAlertThreshold: number;
  onOpenJanitor: () => void;
}

export function StatusFooter({
  status,
  isTauri,
  bridgeStatuses,
  bridgeDeploying,
  onManageBridge,
  janitorHealth,
  ramAlertThreshold,
  onOpenJanitor,
}: StatusFooterProps) {
  const ramElevated = janitorHealth != null && janitorHealth.ram.usedPercent > ramAlertThreshold;
  const janitorAlert = janitorHealth?.alert === true || ramElevated;
  const { t } = useI18n();
  const online = isTauri && status !== null;

  const managed = bridgeStatuses.filter((harness) => harness.installed && harness.supported);
  const bridged = managed.filter((harness) => harness.bridged).length;
  const pending = managed.length - bridged;
  const connected = isTauri && managed.length > 0 && pending === 0;

  const bridgeLabel = !isTauri
    ? t.footer.browserPreview
    : bridgeStatuses.length === 0
      ? t.footer.checking
      : connected
        ? t.footer.connectedAll(bridged)
        : pending === managed.length
          ? t.footer.notConnected
          : t.footer.linkedOf(bridged, managed.length);

  const tone = !isTauri || bridgeStatuses.length === 0 ? "zinc" : connected ? "emerald" : "amber";

  // Solid background on purpose: backdrop-blur over a resizing transparent
  // surface flickers white during maximize, and the footer sits on the app's
  // own canvas so the blur sampled nothing behind it anyway.
  return (
    <footer className="relative z-20 shrink-0 border-t border-white/5 bg-[#0b0b0e] px-4 py-2.5 sm:px-6">
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 text-[11px] text-zinc-500">
        <div className="flex items-center gap-4">
          <span className="flex items-center gap-1.5">
            <span className="relative flex size-1.5">
              <span
                className={cn(
                  "relative inline-flex size-1.5 rounded-full",
                  online ? "bg-emerald-400" : "bg-zinc-600",
                )}
              />
            </span>
            <span className="font-medium text-zinc-300">
              {online ? t.footer.systemOnline : t.footer.previewMode}
            </span>
          </span>
          <span className="hidden items-center gap-1.5 sm:flex">
            <Radio className="size-3" />
            {t.footer.core} {status?.version ?? "0.1.0"}
            <span className="text-zinc-700">·</span>
            {status?.platform ?? "linux-x64"}
          </span>
        </div>

        <div className="hidden items-center gap-1.5 font-mono text-[10.5px] text-zinc-600 lg:flex">
          <FolderTree className="size-3" />
          ~/.orquestra/active
        </div>

        <div className="flex items-center gap-2">
          {isTauri && (
            <button
              type="button"
              onClick={onOpenJanitor}
              title={t.janitor.title}
              aria-label={t.janitor.title}
              className={cn(
                "flex items-center gap-1.5 rounded-md border px-2 py-1 font-mono text-[10.5px] transition focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none",
                janitorAlert
                  ? "border-amber-500/25 bg-amber-500/[0.08] text-amber-300 hover:border-amber-500/40"
                  : "border-cyan-500/20 bg-cyan-500/[0.05] text-cyan-200 hover:border-cyan-500/40",
              )}
            >
              <Activity className="size-3" />
              {janitorHealth ? `${Math.round(janitorHealth.ram.usedPercent)}%` : "—"}
            </button>
          )}
          {isTauri && pending > 0 && (
            <button
              type="button"
              onClick={onManageBridge}
              disabled={bridgeDeploying}
              className="accent-border accent-bg-soft accent-text flex items-center gap-1.5 rounded-md border px-2 py-1 text-[10.5px] font-semibold transition hover:border-[var(--accent-ring)] disabled:opacity-60 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
            >
              {bridgeDeploying ? (
                <Loader2 className="size-3 animate-spin" />
              ) : (
                <PlugZap className="size-3" />
              )}
              {bridgeDeploying ? t.footer.linking : t.footer.connectBridge}
            </button>
          )}

          <button
            type="button"
            onClick={onManageBridge}
            disabled={!isTauri}
            title={isTauri ? t.footer.manageHint : t.footer.previewHint}
            className={cn(
              "flex items-center gap-1.5 rounded-md border px-2 py-1 font-medium transition-colors focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none",
              tone === "emerald" &&
                "accent-border accent-bg-soft accent-text hover:border-[var(--accent-ring)]",
              tone === "amber" &&
                "border-amber-500/25 bg-amber-500/[0.07] text-amber-300 hover:border-amber-500/40",
              tone === "zinc" &&
                "border-white/[0.07] bg-white/[0.03] text-zinc-400 disabled:cursor-default",
            )}
          >
            {tone === "amber" ? <AlertTriangle className="size-3" /> : <Link2 className="size-3" />}
            {t.footer.harnessBridge}
            <span className="text-zinc-700">·</span>
            <span className="font-mono text-[10.5px]">{bridgeLabel}</span>
          </button>
        </div>
      </div>
    </footer>
  );
}
