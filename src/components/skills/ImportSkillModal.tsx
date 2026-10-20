import { useEffect, useState } from "react";
import { CheckCircle2, FolderDown, FolderGit2, Loader2, X } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { importSkill, importSkillFromGit, pickFolder } from "../../lib/tauri";
import type { SkillItem } from "../../lib/skills";
import { cn, confirmOnEnter } from "../../lib/utils";

interface ImportSkillModalProps {
  isTauri: boolean;
  onClose: () => void;
  /** Parent rescans the vault, refreshes counters and toasts success. */
  onImported: (skill: SkillItem) => void;
  onError: (message: string) => void;
}

function toMessage(cause: unknown): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return String(cause);
}

/**
 * Two stacked import paths: a local folder (path input + drag & drop that
 * picks up a dropped folder path) and a GitHub URL (shallow-cloned by the
 * core into /var/tmp/testing, never /tmp).
 */
export function ImportSkillModal({ isTauri, onClose, onImported, onError }: ImportSkillModalProps) {
  const { t } = useI18n();
  const s = t.skills.import;
  const [folderPath, setFolderPath] = useState("");
  const [gitUrl, setGitUrl] = useState("");
  const [busy, setBusy] = useState<"browse" | "folder" | "git" | null>(null);
  const [dragOver, setDragOver] = useState(false);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, onClose]);

  const runBrowse = async () => {
    if (busy) return;
    setBusy("browse");
    try {
      const selected = await pickFolder();
      if (selected) setFolderPath(selected);
    } catch (cause) {
      onError(s.failed(toMessage(cause)));
    } finally {
      setBusy(null);
    }
  };

  const runFolder = async () => {
    const source = folderPath.trim();
    if (!source || busy) return;
    setBusy("folder");
    try {
      onImported(await importSkill(source));
      onClose();
    } catch (cause) {
      onError(s.failed(toMessage(cause)));
    } finally {
      setBusy(null);
    }
  };

  const runGit = async () => {
    const url = gitUrl.trim();
    if (!url || busy) return;
    setBusy("git");
    try {
      onImported(await importSkillFromGit(url));
      onClose();
    } catch (cause) {
      onError(s.failed(toMessage(cause)));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={s.title}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) onClose();
      }}
    >
      <div className="w-full max-w-md overflow-hidden rounded-2xl border border-white/10 bg-[#0e0e12] shadow-[0_30px_80px_-20px_rgba(0,0,0,0.9)]">
        <div className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div>
            <h2 className="text-sm font-semibold text-zinc-100">{s.title}</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">{s.subtitle}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={busy !== null}
            aria-label={t.common.close}
            className="rounded-md p-1.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200 disabled:opacity-40"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="space-y-4 px-5 py-4">
          <section
            onDragOver={(event) => {
              event.preventDefault();
              setDragOver(true);
            }}
            onDragLeave={() => setDragOver(false)}
            onDrop={(event) => {
              event.preventDefault();
              setDragOver(false);
              // Tauri webviews expose the OS path on dropped files; plain
              // browsers do not, so this is best-effort with a typed fallback.
              const file = event.dataTransfer.files?.[0] as (File & { path?: string }) | undefined;
              const path = file?.path;
              if (path) setFolderPath(path);
            }}
            className={cn(
              "rounded-xl border border-dashed p-3.5 transition-[border-color,background-color]",
              dragOver
                ? "border-emerald-500/50 bg-emerald-500/[0.06]"
                : "border-white/10 bg-white/[0.015]",
            )}
          >
            <h3 className="flex items-center gap-1.5 text-[12.5px] font-semibold text-zinc-100">
              <FolderDown className="size-3.5 text-emerald-400" />
              {s.folderTitle}
            </h3>
            <p className="mt-0.5 text-[11px] text-zinc-500">{s.dropHint}</p>
            <button
              type="button"
              onClick={() => void runBrowse()}
              disabled={!isTauri || busy !== null}
              className="mt-2 flex w-full items-center justify-center gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/15 py-2 text-xs font-semibold text-emerald-200 transition hover:bg-emerald-500/25 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {busy === "browse" ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <FolderDown className="size-4" />
              )}
              {s.browseFolder}
            </button>
            {folderPath.trim() && (
              <p className="mt-2 flex min-w-0 items-center gap-1.5 rounded-lg border border-emerald-500/20 bg-emerald-500/[0.06] px-2.5 py-1.5 font-mono text-[11px] text-emerald-200">
                <CheckCircle2 className="size-3.5 shrink-0" />
                <span className="truncate" title={folderPath.trim()}>
                  {s.selectedFolder(folderPath.trim())}
                </span>
              </p>
            )}
            <input
              value={folderPath}
              onChange={(event) => setFolderPath(event.target.value)}
              onKeyDown={(event) => confirmOnEnter(event, () => void runFolder())}
              placeholder={s.pathPlaceholder}
              spellCheck={false}
              disabled={!isTauri || busy !== null}
              className="mt-2 h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-2.5 font-mono text-[12px] text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-emerald-500/40 disabled:opacity-50"
            />
            <button
              type="button"
              onClick={() => void runFolder()}
              disabled={!isTauri || !folderPath.trim() || busy !== null}
              className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-1.5 text-[12px] font-medium text-emerald-200 transition hover:bg-emerald-500/25 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {busy === "folder" && <Loader2 className="size-3.5 animate-spin" />}
              {busy === "folder" ? s.importing : s.folderAction}
            </button>
          </section>

          <section className="rounded-xl border border-white/[0.07] bg-white/[0.015] p-3.5">
            <h3 className="flex items-center gap-1.5 text-[12.5px] font-semibold text-zinc-100">
              <FolderGit2 className="size-3.5 text-zinc-300" />
              {s.gitTitle}
            </h3>
            <input
              value={gitUrl}
              onChange={(event) => setGitUrl(event.target.value)}
              onKeyDown={(event) => confirmOnEnter(event, () => void runGit())}
              placeholder={s.gitPlaceholder}
              spellCheck={false}
              inputMode="url"
              disabled={!isTauri || busy !== null}
              className="mt-2 h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-2.5 font-mono text-[12px] text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-emerald-500/40 disabled:opacity-50"
            />
            <button
              type="button"
              onClick={() => void runGit()}
              disabled={!isTauri || !gitUrl.trim() || busy !== null}
              className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[12px] font-medium text-zinc-200 transition hover:border-white/20 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {busy === "git" && <Loader2 className="size-3.5 animate-spin" />}
              {busy === "git" ? s.importing : s.gitAction}
            </button>
          </section>

          {!isTauri && (
            <p className="text-[11px] leading-relaxed text-zinc-500">{s.unavailable}</p>
          )}
        </div>
      </div>
    </div>
  );
}
