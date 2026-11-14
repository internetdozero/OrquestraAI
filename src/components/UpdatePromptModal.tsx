import { useEffect } from "react";
import { Download, ExternalLink, Loader2, Sparkles, X } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { openExternalUrl } from "../lib/tauri";
import type { ReleaseCheckResult } from "../lib/updater";

interface UpdatePromptModalProps {
  version: string;
  release: ReleaseCheckResult | null;
  installing: boolean;
  onInstallAndRestart: () => void;
  onDismiss: () => void;
}

/** Non-blocking prompt shown after a new release is downloaded and verified. */
export function UpdatePromptModal({
  version,
  release,
  installing,
  onInstallAndRestart,
  onDismiss,
}: UpdatePromptModalProps) {
  const { t } = useI18n();
  const a = t.settings.about;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !installing) onDismiss();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [installing, onDismiss]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !installing) onDismiss();
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-label={a.updatePromptTitle(version)}
        className="w-full max-w-md overflow-hidden rounded-2xl border border-amber-500/25 bg-[#0e0e12] shadow-[0_30px_80px_-20px_rgba(0,0,0,0.9)]"
      >
        <header className="flex items-start justify-between gap-3 border-b border-white/[0.06] px-5 py-4">
          <div className="flex items-center gap-3">
            <div className="grid size-9 place-items-center rounded-lg border border-amber-500/30 bg-amber-500/10 text-amber-300">
              <Sparkles className="size-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-zinc-100">
                {a.updatePromptTitle(version)}
              </h2>
              <p className="text-[11px] text-zinc-400">{a.updatePromptBody(version)}</p>
            </div>
          </div>
          <button
            type="button"
            disabled={installing}
            onClick={onDismiss}
            aria-label={a.later}
            className="rounded-md p-1.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200 disabled:opacity-40"
          >
            <X className="size-4" />
          </button>
        </header>

        {release?.releaseNotes && (
          <div className="max-h-44 overflow-y-auto border-b border-white/[0.06] bg-white/[0.015] px-5 py-3">
            <pre className="whitespace-pre-wrap font-sans text-[11.5px] leading-relaxed text-zinc-300">
              {release.releaseNotes.trim()}
            </pre>
          </div>
        )}

        <footer className="flex flex-wrap items-center justify-between gap-2 px-5 py-3.5">
          {release?.releaseUrl ? (
            <button
              type="button"
              onClick={() => void openExternalUrl(release.releaseUrl as string)}
              className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-2.5 py-1.5 text-[11.5px] font-medium text-zinc-300 transition hover:border-white/20"
            >
              <ExternalLink className="size-3.5" />
              {a.viewRelease}
            </button>
          ) : (
            <span />
          )}

          <div className="flex items-center gap-2">
            <button
              type="button"
              disabled={installing}
              onClick={onDismiss}
              className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-300 transition hover:border-white/20 disabled:opacity-40"
            >
              {a.later}
            </button>
            <button
              type="button"
              disabled={installing}
              onClick={onInstallAndRestart}
              className="flex items-center gap-1.5 rounded-lg border border-amber-500/40 bg-amber-500/15 px-3 py-1.5 text-[11.5px] font-semibold text-amber-200 transition hover:bg-amber-500/25 disabled:cursor-wait disabled:opacity-60"
            >
              {installing ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Download className="size-3.5" />
              )}
              {installing ? a.installingUpdate : a.installNow}
            </button>
          </div>
        </footer>
      </section>
    </div>
  );
}
