import { useState } from "react";
import { FolderOpen, HardDrive, Save } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { createOrquestraBackup, openInFileManager } from "../../lib/tauri";

const PATHS = [
  { key: "root", path: "~/.orquestra" },
  { key: "vault", path: "~/.orquestra/skills-library" },
  { key: "active", path: "~/.orquestra/active" },
  { key: "master", path: "~/.orquestra/AGENTS.md" },
] as const;

/** Read-only canonical paths plus native open-folder and backup actions. */
export function StorageSettingsTab({
  isTauri,
  onToast,
  onError,
}: {
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const s = t.settings.storage;
  const [busy, setBusy] = useState(false);

  const openFolder = async () => {
    if (!isTauri) return;
    try {
      await openInFileManager("~/.orquestra");
      onToast(s.opened);
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      onError(s.openFailed(detail));
    }
  };

  const backup = async () => {
    if (!isTauri || busy) return;
    setBusy(true);
    try {
      const archive = await createOrquestraBackup();
      onToast(s.backedUpTo(archive));
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      onError(s.backupFailed(detail));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-4">
      <div>
        <h3 className="flex items-center gap-1.5 text-[12.5px] font-semibold text-zinc-100">
          <HardDrive className="size-3.5 text-emerald-400" />
          {s.pathsTitle}
        </h3>
        <p className="mt-0.5 text-[11px] text-zinc-500">{s.pathsHint}</p>
      </div>
      <ul className="space-y-1.5">
        {PATHS.map((entry) => (
          <li
            key={entry.key}
            className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.06] bg-white/[0.015] px-3 py-2"
          >
            <span className="text-[12px] text-zinc-400">
              {s[entry.key as "root" | "vault" | "active" | "master"]}
            </span>
            <code className="truncate font-mono text-[11px] text-zinc-200">{entry.path}</code>
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => void openFolder()}
          disabled={!isTauri}
          className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-200 transition hover:border-white/20 disabled:opacity-50"
        >
          <FolderOpen className="size-3.5 text-emerald-400" />
          {s.openFolder}
        </button>
        <button
          type="button"
          onClick={() => void backup()}
          disabled={!isTauri || busy}
          className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-200 transition hover:border-white/20 disabled:opacity-50"
        >
          <Save className="size-3.5 text-cyan-300" />
          {s.backup}
        </button>
      </div>
    </div>
  );
}
