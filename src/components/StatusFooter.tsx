import { AlertTriangle, FolderTree, Link2, Loader2, PlugZap, Radio } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import type { HarnessStatus } from "../lib/bridge";
import type { CoreStatus } from "../lib/tauri";
import { cn } from "../lib/utils";
import { LanguageToggle } from "./LanguageToggle";

interface StatusFooterProps {
  status: CoreStatus | null;
  isTauri: boolean;
  bridgeStatuses: HarnessStatus[];
  bridgeDeploying: boolean;
  onManageBridge: () => void;
}

export function StatusFooter({
  status,
  isTauri,
  bridgeStatuses,
  bridgeDeploying,
  onManageBridge,
}: StatusFooterProps) {
  const { t } = useI18n();
  const online = isTauri && status !== null;

  const managed = bridgeStatuses.filter(
    (harness) => harness.installed && harness.supported,
  );
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

  const tone = !isTauri || bridgeStatuses.length === 0
    ? "zinc"
    : connected
      ? "emerald"
      : "amber";

  return (
    <footer className="relative z-20 shrink-0 border-t border-white/5 bg-[#0b0b0e]/85 px-4 py-2.5 backdrop-blur-xl sm:px-6">
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 text-[11px] text-zinc-500">
        <div className="flex items-center gap-4">
          <span className="flex items-center gap-1.5">
            <span className="relative flex size-1.5">
              {online && (
                <span className="absolute inline-flex size-full animate-pulse rounded-full bg-emerald-400 opacity-75" />
              )}
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
          <LanguageToggle />

          {isTauri && pending > 0 && (
            <button
              type="button"
              onClick={onManageBridge}
              disabled={bridgeDeploying}
              className="flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/[0.12] px-2 py-1 text-[10.5px] font-semibold text-emerald-200 transition hover:border-emerald-400/50 hover:bg-emerald-500/20 disabled:opacity-60 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
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
                "border-emerald-500/25 bg-emerald-500/[0.08] text-emerald-300 hover:border-emerald-500/40",
              tone === "amber" &&
                "border-amber-500/25 bg-amber-500/[0.07] text-amber-300 hover:border-amber-500/40",
              tone === "zinc" &&
                "border-white/[0.07] bg-white/[0.03] text-zinc-400 disabled:cursor-default",
            )}
          >
            {tone === "amber" ? (
              <AlertTriangle className="size-3" />
            ) : (
              <Link2 className="size-3" />
            )}
            {t.footer.harnessBridge}
            <span className="text-zinc-700">·</span>
            <span className="font-mono text-[10.5px]">{bridgeLabel}</span>
          </button>
        </div>
      </div>
    </footer>
  );
}
