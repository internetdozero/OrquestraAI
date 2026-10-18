import { useEffect, useState } from "react";
import { Copy, Loader2, MessagesSquare } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { getSessionTranscript, type TranscriptTurn } from "../../lib/sessions";
import { cn } from "../../lib/utils";

/** One chat bubble with a hover copy affordance. */
function Turn({ turn }: { turn: TranscriptTurn }) {
  const { t } = useI18n();
  const s = t.sessions;
  const isUser = turn.role === "user";

  return (
    <div className={cn("group flex", isUser ? "justify-end" : "justify-start")}>
      <div
        className={cn(
          "relative max-w-[92%] rounded-xl border px-2.5 py-1.5",
          isUser
            ? "border-emerald-500/25 bg-emerald-500/[0.08]"
            : "border-white/[0.07] bg-white/[0.025]",
        )}
      >
        <p
          className={cn(
            "mb-1 font-mono text-[9px] font-semibold tracking-[0.12em] uppercase",
            isUser ? "text-right text-emerald-400/80" : "text-zinc-600",
          )}
        >
          {isUser ? s.turnUser : turn.role === "system" ? s.turnSystem : s.turnAssistant}
          {turn.timestamp && (
            <span className="ml-1.5 font-normal tracking-normal normal-case opacity-70">
              {turn.timestamp}
            </span>
          )}
        </p>
        <p className="selectable-text font-sans text-[12px] leading-relaxed whitespace-pre-wrap text-zinc-200">
          {turn.content}
        </p>
        <button
          type="button"
          title={s.copyMessage}
          aria-label={s.copyMessage}
          onClick={() => {
            void navigator.clipboard.writeText(turn.content).catch(() => undefined);
          }}
          className="absolute top-1 right-1 grid size-5 place-items-center rounded bg-black/50 text-zinc-500 opacity-0 transition group-hover:opacity-100 hover:text-emerald-300 focus-visible:opacity-100"
        >
          <Copy className="size-3" />
        </button>
      </div>
    </div>
  );
}

/**
 * Real conversation thread for one session. Loading spinners, empty states
 * and per-harness gaps are all handled here so the drawer stays thin.
 */
export function TranscriptFeed({
  harness,
  sessionId,
}: {
  harness: string;
  sessionId: string;
}) {
  const { t } = useI18n();
  const s = t.sessions;
  const [turns, setTurns] = useState<TranscriptTurn[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    setTurns(null);
    void getSessionTranscript(harness, sessionId)
      .then((feed) => {
        if (!cancelled) setTurns(feed);
      })
      .catch(() => {
        if (!cancelled) setTurns([]);
      });
    return () => {
      cancelled = true;
    };
  }, [harness, sessionId]);

  if (turns === null) {
    return (
      <div className="flex items-center justify-center gap-2 py-8 text-[11.5px] text-zinc-500">
        <Loader2 className="size-3.5 animate-spin" />
        {s.transcriptLoading}
      </div>
    );
  }

  if (turns.length === 0) {
    return (
      <div className="flex flex-col items-center gap-1.5 rounded border border-dashed border-white/[0.08] bg-black/20 px-3 py-6 text-center">
        <MessagesSquare className="size-4 text-zinc-600" />
        <p className="max-w-[26ch] text-[11px] leading-relaxed text-zinc-500">
          {s.transcriptEmpty}
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {turns.map((turn, index) => (
        <Turn key={`${index}-${turn.role}-${turn.content.length}`} turn={turn} />
      ))}
    </div>
  );
}
