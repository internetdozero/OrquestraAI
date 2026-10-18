import { Check, Inbox, Megaphone } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { BlackboardNote } from "../../lib/ideas";
import { StatePanel } from "../ui";
import { cn } from "../../lib/utils";

interface BlackboardFeedProps {
  notes: BlackboardNote[];
  loading: boolean;
  onMarkRead: (id: number) => void;
}

/** Inter-agent notes: sender → target, topic chip, content, read action. */
export function BlackboardFeed({ notes, loading, onMarkRead }: BlackboardFeedProps) {
  const { t } = useI18n();
  const s = t.ideas;

  if (!loading && notes.length === 0) {
    return <StatePanel icon={Inbox} title={s.emptyNotes} description={s.emptyNotesHint} />;
  }

  return (
    <ol className="space-y-2">
      {notes.map((note) => {
        const stamped = new Date(note.createdAt.replace(" ", "T") + "Z");
        return (
          <li
            key={note.id}
            className={cn(
              "rounded-xl border p-3 transition",
              note.isRead
                ? "border-white/[0.05] bg-zinc-900/20"
                : "border-amber-500/20 bg-amber-500/[0.03]",
            )}
          >
            <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px]">
              <Megaphone
                className={cn("size-3.5", note.isRead ? "text-zinc-600" : "text-amber-400")}
                aria-hidden="true"
              />
              <span className="text-zinc-500">{s.noteFrom}</span>
              <span className="font-mono font-medium text-zinc-200">{note.sender}</span>
              <span className="text-zinc-600" aria-hidden="true">
                →
              </span>
              <span className="text-zinc-500">{s.noteTo}</span>
              <span className="font-mono font-medium text-zinc-200">{note.target}</span>
              <span className="rounded border border-cyan-500/25 bg-cyan-500/10 px-1.5 py-px font-mono text-[10px] text-cyan-300">
                {note.topic}
              </span>
              <span className="rounded border border-white/10 bg-white/[0.04] px-1.5 py-px font-mono text-[10px] text-zinc-400">
                {note.project}
              </span>
              {!Number.isNaN(stamped.getTime()) && (
                <span className="ml-auto text-[10px] text-zinc-600">
                  {stamped.toLocaleString()}
                </span>
              )}
            </div>
            <p className="mt-1.5 text-[12.5px] leading-relaxed whitespace-pre-wrap text-zinc-300">
              {note.note}
            </p>
            <div className="mt-2">
              {note.isRead ? (
                <span className="flex w-fit items-center gap-1 rounded-md border border-white/[0.07] px-2 py-1 text-[10.5px] font-medium text-zinc-500">
                  <Check className="size-3" aria-hidden="true" />
                  {s.read}
                </span>
              ) : (
                <button
                  type="button"
                  onClick={() => onMarkRead(note.id)}
                  className="flex items-center gap-1 rounded-md border border-amber-500/30 bg-amber-500/10 px-2 py-1 text-[10.5px] font-medium text-amber-200 transition hover:bg-amber-500/20"
                >
                  <Check className="size-3" aria-hidden="true" />
                  {s.markRead}
                </button>
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}
