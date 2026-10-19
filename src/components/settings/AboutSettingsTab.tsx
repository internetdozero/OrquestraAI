import { useState } from "react";
import {
  Check,
  CheckCircle2,
  ClipboardCopy,
  Download,
  ExternalLink,
  Loader2,
  Orbit,
  RefreshCw,
  RotateCcw,
} from "lucide-react";
import { useAppUpdater } from "../../hooks/useAppUpdater";
import { useClipboard } from "../../hooks/useClipboard";
import { useI18n } from "../../hooks/useI18n";
import { HARNESSES } from "../../lib/harnesses";
import type { CoreStatus } from "../../lib/tauri";
import { openExternalUrl } from "../../lib/tauri";
import { checkLatestRelease, type ReleaseCheckResult } from "../../lib/updater";
import { cn } from "../../lib/utils";

interface AboutSettingsTabProps {
  status: CoreStatus | null;
  isTauri: boolean;
  autoCheckAppUpdates: boolean;
  autoCheckExtensionUpdates: boolean;
  onToggleAppUpdates: () => void;
  onToggleExtensionUpdates: () => void;
  onToast: (message: string) => void;
  onError: (message: string) => void;
}

/** Version, platform info and a clipboard diagnostics snippet. */
export function AboutSettingsTab({
  status,
  isTauri,
  autoCheckAppUpdates,
  autoCheckExtensionUpdates,
  onToggleAppUpdates,
  onToggleExtensionUpdates,
  onToast,
  onError,
}: AboutSettingsTabProps) {
  const { t } = useI18n();
  const a = t.settings.about;
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<ReleaseCheckResult | null>(null);
  const { copied: copiedDiag, copy: copyDiagText } = useClipboard(1500);
  const version = status?.version ?? "0.1.0";
  const { updatingApp, syncing, updatedTo, updateApp, syncExtensions, restart } =
    useAppUpdater({ onToast, onError });

  const checkNow = async () => {
    setChecking(true);
    setResult(null);
    try {
      setResult(await checkLatestRelease(version));
    } finally {
      setChecking(false);
    }
  };

  const copyDiagnostics = async () => {
    const lines = [
      "# OrquestraAI diagnostics",
      `- version: ${status?.version ?? "0.1.0"}`,
      `- platform: ${status?.platform ?? "browser-preview"}`,
      `- core: ${status?.name ?? "orquestra-ai"}`,
      `- tauri: ${isTauri ? "desktop" : "browser-preview"}`,
      `- harnesses: ${HARNESSES.map((h) => h.id).join(", ")}`,
      `- bridge: ${status?.bridge ?? "unknown"}`,
      `- date: ${new Date().toISOString()}`,
    ];
    try {
      const ok = await copyDiagText(lines.join("\n"));
      if (ok) onToast(a.copied);
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      onError(a.copyFailed(detail));
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3 rounded-xl border border-white/[0.07] bg-white/[0.02] p-3.5">
        <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-gradient-to-br from-violet-400/20 via-purple-500/10 to-indigo-500/10 ring-1 ring-violet-400/25">
          <Orbit className="size-5 text-violet-300" />
        </div>
        <div>
          <p className="text-[13px] font-semibold text-zinc-100">
            Orquestra<span className="text-violet-400">AI</span>
          </p>
          <p className="font-mono text-[10.5px] text-zinc-500">
            {a.version} {status?.version ?? "0.1.0"} · {a.platform}{" "}
            {status?.platform ?? "browser-preview"}
          </p>
        </div>
      </div>

      <dl className="space-y-1.5">
        {[
          [a.core, status?.name ?? "orquestra-ai"],
          ["Bridge", status?.bridge ?? "—"],
          ["Harnesses", `${HARNESSES.length}`],
        ].map(([term, value]) => (
          <div
            key={term}
            className="flex items-center justify-between rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2"
          >
            <dt className="text-[12px] text-zinc-500">{term}</dt>
            <dd className="font-mono text-[11.5px] text-zinc-200">{value}</dd>
          </div>
        ))}
      </dl>

      <button
        type="button"
        onClick={() => void copyDiagnostics()}
        className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-200 transition hover:border-white/20"
      >
        {copiedDiag ? <Check className="size-3.5 text-emerald-400" /> : <ClipboardCopy className="size-3.5 text-emerald-400" />}
        {a.copyDiag}
      </button>

      <section>
        <h3 className="text-[12.5px] font-semibold text-zinc-100">{a.updatesTitle}</h3>
        <div className="mt-2 space-y-2">
          {[
            {
              title: a.autoCheckAppTitle,
              hint: a.autoCheckAppDesc,
              checked: autoCheckAppUpdates,
              onFlip: onToggleAppUpdates,
            },
            {
              title: a.autoCheckExtTitle,
              hint: a.autoCheckExtDesc,
              checked: autoCheckExtensionUpdates,
              onFlip: onToggleExtensionUpdates,
            },
          ].map((row) => (
            <div
              key={row.title}
              className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.07] bg-white/[0.02] px-3 py-2.5"
            >
              <div>
                <p className="text-[12.5px] text-zinc-200">{row.title}</p>
                <p className="text-[11px] text-zinc-500">{row.hint}</p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={row.checked}
                aria-label={row.title}
                onClick={row.onFlip}
                className={cn(
                  "relative h-5.5 w-10 shrink-0 rounded-full transition-[background-color]",
                  row.checked ? "accent-dot" : "bg-zinc-700",
                )}
              >
                <span
                  className={cn(
                    "absolute top-0.5 size-4.5 rounded-full bg-white transition-[left]",
                    row.checked ? "left-5" : "left-0.5",
                  )}
                />
              </button>
            </div>
          ))}
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={syncing || !isTauri}
            onClick={() => void syncExtensions()}
            className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-200 transition hover:border-white/20 disabled:cursor-wait disabled:opacity-60"
          >
            {syncing ? (
              <Loader2 className="size-3.5 animate-spin text-emerald-400" />
            ) : (
              <Download className="size-3.5 text-emerald-400" />
            )}
            {syncing ? a.syncingExtensions : a.syncExtensions}
          </button>
          <button
            type="button"
            disabled={checking}
            onClick={() => void checkNow()}
            className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-200 transition hover:border-white/20 disabled:cursor-wait disabled:opacity-60"
          >
            {checking ? (
              <Loader2 className="size-3.5 animate-spin text-emerald-400" />
            ) : (
              <RefreshCw className="size-3.5 text-emerald-400" />
            )}
            {checking ? a.checking : a.checkNow}
          </button>
          {result && !checking && result.isLatest && (
            <span
              role="status"
              className="flex items-center gap-1.5 rounded-full border border-emerald-500/25 bg-emerald-500/10 px-2.5 py-1 text-[11px] font-medium text-emerald-200"
            >
              <CheckCircle2 className="size-3" />
              {a.upToDate(version)}
            </span>
          )}
          {updatedTo && !checking && (
            <div
              role="status"
              className="flex flex-wrap items-center gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-[11.5px] font-medium text-emerald-200"
            >
              <CheckCircle2 className="size-3.5" />
              <span>{a.updateAppSuccess}</span>
              <button
                type="button"
                onClick={restart}
                className="flex items-center gap-1 rounded-md border border-emerald-500/40 bg-emerald-500/15 px-2 py-0.5 text-[11px] transition hover:bg-emerald-500/25"
              >
                <RotateCcw className="size-3" />
                {a.restart}
              </button>
            </div>
          )}
          {result && !checking && !result.isLatest && !updatedTo && (
            <div
              role="status"
              className="flex flex-wrap items-center gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-[11.5px] font-medium text-amber-200"
            >
              <span>{a.newVersionAvailable(result.latestVersion)}</span>
              <button
                type="button"
                disabled={updatingApp || !isTauri}
                onClick={() => void updateApp(result.latestVersion)}
                className="flex items-center gap-1 rounded-md border border-amber-500/40 bg-amber-500/15 px-2 py-0.5 text-[11px] transition hover:bg-amber-500/25 disabled:cursor-wait disabled:opacity-60"
              >
                {updatingApp ? (
                  <Loader2 className="size-3 animate-spin" />
                ) : (
                  <Download className="size-3" />
                )}
                {updatingApp ? a.updatingApp : a.updateNow}
              </button>
              {result.releaseUrl && (
                <button
                  type="button"
                  onClick={() => void openExternalUrl(result.releaseUrl as string)}
                  className="flex items-center gap-1 rounded-md border border-amber-500/40 bg-amber-500/15 px-2 py-0.5 text-[11px] transition hover:bg-amber-500/25"
                >
                  <ExternalLink className="size-3" />
                  {a.viewRelease}
                </button>
              )}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
