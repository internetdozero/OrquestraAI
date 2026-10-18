import { Eye, FolderGit2, Pin, Trash2 } from "lucide-react";
import { scopeLabel } from "../../lib/harnesses";
import { useI18n } from "../../hooks/useI18n";
import type { UnifiedSession } from "../../lib/sessions";
import {
  absoluteDate,
  formatBytes,
  formatDateTime,
  harnessColor,
  relativeAge,
  shortPath,
} from "../../lib/sessionFormat";
import { cn } from "../../lib/utils";

interface SessionRowProps {
  session: UnifiedSession;
  busy: boolean;
  onTogglePin: () => void;
  onDelete: () => void;
  onInspect: () => void;
}

/** One compact session card: harness badge, title, cwd, age, size, actions. */
export function SessionRow({ session, busy, onTogglePin, onDelete, onInspect }: SessionRowProps) {
  const { t, locale } = useI18n();
  const s = t.sessions;

  return (
    <article className="group flex items-center gap-3 rounded-xl border border-white/[0.06] bg-zinc-900/30 px-3 py-2.5 transition hover:border-white/[0.12]">
      <span
        className={cn(
          "shrink-0 rounded-md border px-1.5 py-0.5 font-mono text-[10px] font-medium",
          harnessColor(session.harness),
        )}
        title={session.harness}
      >
        {scopeLabel(session.harness)}
      </span>

      <div className="min-w-0 flex-1">
        <button
          type="button"
          onClick={onInspect}
          title={s.inspectTitle}
          className="block w-full truncate text-left text-[12.5px] font-medium text-zinc-100 transition hover:text-emerald-200"
        >
          {session.isPinned && <span className="mr-1 text-amber-300">📌</span>}
          {session.title || session.id}
        </button>
        <p
          className="flex min-w-0 items-center gap-1 truncate font-mono text-[10.5px] text-zinc-500"
          title={session.projectPath ?? session.id}
        >
          <FolderGit2 className="size-3 shrink-0" />
          <span className="truncate">{shortPath(session.projectPath)}</span>
        </p>
      </div>

      <div className="hidden shrink-0 text-right sm:block">
        <p className="font-mono text-[11px] text-zinc-300" title={absoluteDate(session.updatedAt, locale)}>
          {formatDateTime(session.updatedAt)}
        </p>
        <p className="text-[10px] text-zinc-500">
          {relativeAge(session.updatedAt, locale)} · {session.turnsCount} turns
        </p>
      </div>

      <span className="shrink-0 rounded-md border border-white/10 bg-white/[0.03] px-1.5 py-0.5 font-mono text-[10.5px] text-zinc-300">
        {formatBytes(session.sizeBytes)}
      </span>

      <div className="flex shrink-0 items-center gap-1">
        <button
          type="button"
          onClick={onInspect}
          title={s.inspectTitle}
          aria-label={s.inspectTitle}
          className="grid size-7 place-items-center rounded-md border border-white/10 bg-white/[0.02] text-zinc-500 opacity-0 transition group-hover:opacity-100 hover:text-emerald-200 focus-visible:opacity-100"
        >
          <Eye className="size-3.5" />
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={onTogglePin}
          title={session.isPinned ? s.unpinTitle : s.pinTitle}
          aria-label={session.isPinned ? s.unpinTitle : s.pinTitle}
          aria-pressed={session.isPinned}
          className={cn(
            "grid size-7 place-items-center rounded-md border transition disabled:opacity-50",
            session.isPinned
              ? "border-amber-500/40 bg-amber-500/10 text-amber-300"
              : "border-white/10 bg-white/[0.02] text-zinc-500 hover:text-amber-200",
          )}
        >
          <Pin className={cn("size-3.5", session.isPinned && "fill-current")} />
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => {
            if (window.confirm(s.deleteConfirm(session.title || session.id))) onDelete();
          }}
          title={s.deleteTitle}
          aria-label={s.deleteTitle}
          className="grid size-7 place-items-center rounded-md border border-white/10 bg-white/[0.02] text-zinc-500 transition hover:border-rose-500/40 hover:text-rose-300 disabled:opacity-50"
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>
    </article>
  );
}
