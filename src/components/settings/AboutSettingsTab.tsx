import { useEffect, useState } from "react";
import {
  Check,
  CheckCircle2,
  ClipboardCopy,
  Download,
  ExternalLink,
  Loader2,
  RefreshCw,
  RotateCcw,
} from "lucide-react";
import type { AppUpdaterState } from "../../hooks/useAppUpdater";
import { useClipboard } from "../../hooks/useClipboard";
import { useI18n } from "../../hooks/useI18n";
import { HARNESSES } from "../../lib/harnesses";
import type { CoreStatus } from "../../lib/tauri";
import { openExternalUrl } from "../../lib/tauri";
import { probeExternalTools, type ToolReport } from "../../lib/tooling";
import { cn } from "../../lib/utils";
import ownhubLogo from "../../assets/ownhub-logo.svg";

interface AboutSettingsTabProps {
  status: CoreStatus | null;
  isTauri: boolean;
  autoCheckAppUpdates: boolean;
  autoCheckExtensionUpdates: boolean;
  onToggleAppUpdates: () => void;
  onToggleExtensionUpdates: () => void;
  updater: AppUpdaterState;
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
  updater,
  onToast,
  onError,
}: AboutSettingsTabProps) {
  const { t } = useI18n();
  const a = t.settings.about;
  const { copied: copiedDiag, copy: copyDiagText } = useClipboard(1500);
  const [tools, setTools] = useState<ToolReport | null>(null);
  const version = status?.version ?? "0.1.0";
  const {
    checking,
    checkResult,
    availableRelease,
    updatingApp,
    syncing,
    progress,
    readyVersion,
    updatedTo,
    checkNow,
    updateApp,
    installReadyAndRestart,
    syncExtensions,
    restart,
    step,
  } = updater;

  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    probeExternalTools()
      .then((report) => {
        if (!cancelled) setTools(report);
      })
      // A probe failure only means "no report"; the buttons below still fail
      // with their own message when the tool is actually needed.
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [isTauri]);

  const progressPct =
    step === "downloading" && progress?.totalBytes
      ? Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100))
      : null;

  const progressLabel =
    step === "verifying"
      ? a.verifyingUpdate
      : step === "installing"
        ? a.installingUpdate
        : progressPct !== null && progress?.totalBytes
          ? `${a.downloadingUpdate} ${progressPct}% (${(progress.downloadedBytes / 1048576).toFixed(1)} / ${(progress.totalBytes / 1048576).toFixed(1)} MB)`
          : a.downloadingUpdate;

  const copyDiagnostics = async () => {
    const lines = [
      "# OwnHub diagnostics",
      `- version: ${status?.version ?? "0.1.0"}`,
      `- platform: ${status?.platform ?? "browser-preview"}`,
      `- core: ${status?.name ?? "ownhub"}`,
      `- tauri: ${isTauri ? "desktop" : "browser-preview"}`,
      `- harnesses: ${HARNESSES.map((h) => h.id).join(", ")}`,
      `- bridge: ${status?.bridge ?? "unknown"}`,
      `- tools: ${
        tools
          ? tools.tools
              .map((tool) => `${tool.id}=${tool.present ? (tool.version ?? "present") : "missing"}`)
              .join("; ")
          : "unknown"
      }`,
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
        <img
          src={ownhubLogo}
          alt="OwnHub"
          className="size-10 shrink-0 rounded-xl"
          draggable={false}
        />
        <div>
          <p className="text-[13px] font-semibold text-zinc-100">OwnHub</p>
          <p className="font-mono text-[10.5px] text-zinc-500">
            {a.version} {status?.version ?? "0.1.0"} · {a.platform}{" "}
            {status?.platform ?? "browser-preview"}
          </p>
        </div>
      </div>

      <dl className="space-y-1.5">
        {[
          [a.core, status?.name ?? "ownhub"],
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

      <section>
        <h3 className="text-[12.5px] font-semibold text-zinc-100">{a.toolsTitle}</h3>
        {tools && tools.missing.length > 0 && (
          <p
            role="status"
            className="mt-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-[11.5px] text-amber-200"
          >
            {a.toolsMissing(tools.missing.join(", "))}
          </p>
        )}
        {tools?.note && (
          <p
            role="status"
            className="mt-2 rounded-lg border border-sky-500/30 bg-sky-500/10 px-3 py-2 text-[11.5px] text-sky-200"
          >
            {tools.note}
          </p>
        )}
        <ul className="mt-2 space-y-1.5">
          {(tools?.tools ?? []).map((tool) => (
            <li
              key={tool.id}
              className="rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2"
            >
              <div className="flex items-center justify-between gap-3">
                <span className="font-mono text-[11.5px] text-zinc-200">{tool.id}</span>
                <span
                  className={cn(
                    "font-mono text-[11px]",
                    tool.present ? "text-emerald-300" : "text-amber-300",
                  )}
                >
                  {tool.present ? (tool.version ?? a.toolsAvailable) : a.toolsAbsent}
                </span>
              </div>
              <p className="mt-0.5 text-[11px] text-zinc-500">
                {tool.disables.map((feature) => a.toolFeature(feature)).join(" · ")}
              </p>
              {!tool.present && tool.hint && (
                <p className="mt-0.5 text-[10.5px] text-amber-200/80">{tool.hint}</p>
              )}
            </li>
          ))}
        </ul>
        {tools && tools.missing.length === 0 && (
          <p className="mt-2 text-[11px] text-zinc-500">{a.toolsAllPresent}</p>
        )}
      </section>

      <button
        type="button"
        onClick={() => void copyDiagnostics()}
        className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-200 transition hover:border-white/20"
      >
        {copiedDiag ? (
          <Check className="size-3.5 text-emerald-400" />
        ) : (
          <ClipboardCopy className="size-3.5 text-emerald-400" />
        )}
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
          {updatingApp && (
            <div
              role="status"
              aria-live="polite"
              className="mt-2 w-full rounded-lg border border-sky-500/30 bg-sky-500/10 px-3 py-2.5"
            >
              <p className="flex items-center gap-2 text-[11.5px] font-medium text-sky-200">
                <Loader2 className="size-3.5 shrink-0 animate-spin" />
                <span>{progressLabel}</span>
              </p>
              <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-sky-500/15">
                {progressPct !== null ? (
                  <div
                    className="h-full rounded-full bg-sky-400 transition-[width] duration-200"
                    style={{ width: `${progressPct}%` }}
                  />
                ) : (
                  <div className="h-full w-1/3 animate-[indeterminate_1.2s_ease-in-out_infinite] rounded-full bg-sky-400" />
                )}
              </div>
            </div>
          )}
          {checkResult && !checking && checkResult.isLatest && !readyVersion && !updatedTo && (
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
          {readyVersion && !checking && !updatingApp && !updatedTo && (
            <div
              role="status"
              className="flex flex-wrap items-center gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-[11.5px] font-medium text-amber-200"
            >
              <span>{a.readyToInstall(readyVersion)}</span>
              <button
                type="button"
                disabled={updatingApp || !isTauri}
                onClick={() => void installReadyAndRestart()}
                className="flex items-center gap-1 rounded-md border border-amber-500/40 bg-amber-500/15 px-2 py-0.5 text-[11px] font-semibold transition hover:bg-amber-500/25 disabled:cursor-wait disabled:opacity-60"
              >
                <Download className="size-3" />
                {a.installNow}
              </button>
              {availableRelease?.releaseUrl && (
                <button
                  type="button"
                  onClick={() => void openExternalUrl(availableRelease.releaseUrl as string)}
                  className="flex items-center gap-1 rounded-md border border-amber-500/40 bg-amber-500/15 px-2 py-0.5 text-[11px] transition hover:bg-amber-500/25"
                >
                  <ExternalLink className="size-3" />
                  {a.viewRelease}
                </button>
              )}
            </div>
          )}
          {availableRelease &&
            !availableRelease.isLatest &&
            !readyVersion &&
            !checking &&
            !updatingApp &&
            !updatedTo && (
              <div
                role="status"
                className="flex flex-wrap items-center gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-[11.5px] font-medium text-amber-200"
              >
                <span>{a.newVersionAvailable(availableRelease.latestVersion)}</span>
                <button
                  type="button"
                  disabled={updatingApp || !isTauri}
                  onClick={() => void updateApp(availableRelease.latestVersion)}
                  className="flex items-center gap-1 rounded-md border border-amber-500/40 bg-amber-500/15 px-2 py-0.5 text-[11px] transition hover:bg-amber-500/25 disabled:cursor-wait disabled:opacity-60"
                >
                  <Download className="size-3" />
                  {a.updateNow}
                </button>
                {availableRelease.releaseUrl && (
                  <button
                    type="button"
                    onClick={() => void openExternalUrl(availableRelease.releaseUrl as string)}
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
