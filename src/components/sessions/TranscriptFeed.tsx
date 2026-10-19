import { useEffect, useMemo, useState } from "react";
import { Check, Copy, Download, Loader2, MessagesSquare, Search, X } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { useClipboard } from "../../hooks/useClipboard";
import { getSessionTranscript, type TranscriptTurn } from "../../lib/sessions";
import { cn } from "../../lib/utils";
import { MarkdownTurn } from "./MarkdownTurn";

export interface TranscriptMetadata {
  harness: string;
  id: string;
  title?: string;
}

/** Whole thread as a clean Markdown document, ready to copy or download. */
export function formatTranscriptMarkdown(
  turns: TranscriptTurn[],
  metadata: TranscriptMetadata,
): string {
  const lines = [
    `# Session Transcript: ${metadata.title || metadata.id}`,
    `- **Harness:** ${metadata.harness}`,
    `- **Session ID:** ${metadata.id}`,
    `- **Exported At:** ${new Date().toISOString()}`,
    `- **Turns:** ${turns.length}`,
    "---",
  ];
  for (const turn of turns) {
    const role = turn.role.charAt(0).toUpperCase() + turn.role.slice(1);
    lines.push(`### ${role}${turn.timestamp ? ` · ${turn.timestamp}` : ""}`, turn.content, "");
  }
  return lines.join("\n");
}

/** One chat bubble with a hover copy affordance. */
function Turn({ turn, onToast }: { turn: TranscriptTurn; onToast: (message: string) => void }) {
  const { t } = useI18n();
  const s = t.sessions;
  const { copied, copy } = useClipboard(1500);
  const isUser = turn.role === "user";

  return (
    <div
      data-turn-role={turn.role}
      className={cn("group flex", isUser ? "justify-end" : "justify-start")}
    >
      <div
        className={cn(
          "relative min-w-0 max-w-[92%] rounded-xl border px-2.5 py-1.5 break-words [overflow-wrap:anywhere]",
          isUser
            ? "border-emerald-500/25 bg-emerald-500/[0.08]"
            : "border-white/[0.07] bg-white/[0.025]",
        )}
      >
        <p
          className={cn(
            // `pr-5` keeps the timestamp clear of the hover copy button.
            "mb-1 pr-5 font-mono text-[9px] font-semibold tracking-[0.12em] uppercase",
            isUser ? "text-right text-emerald-400/80" : "text-zinc-600",
          )}
        >
          {isUser ? s.turnUser : s.turnAssistant}
          {turn.timestamp && (
            <span className="ml-1.5 font-normal tracking-normal normal-case opacity-70">
              {turn.timestamp}
            </span>
          )}
        </p>
        <MarkdownTurn content={turn.content} role={turn.role} />
        <button
          type="button"
          title={copied ? s.copied : s.copyMessage}
          aria-label={copied ? s.copied : s.copyMessage}
          onClick={async () => {
            const ok = await copy(turn.content);
            if (ok) onToast(s.copied);
          }}
          className={cn(
            "absolute top-1 right-1 grid size-5 place-items-center rounded bg-black/50 pointer-events-none group-hover:pointer-events-auto hover:text-emerald-300 focus-visible:pointer-events-auto",
            copied
              ? "text-emerald-300 opacity-100"
              : "text-zinc-500 opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
          )}
        >
          {copied ? <Check className="size-3 text-emerald-300" /> : <Copy className="size-3" />}
        </button>
      </div>
    </div>
  );
}

/**
 * Real conversation thread for one session: keyword search, copy-all and
 * Markdown export on top; loading, empty and per-harness gaps below.
 */
export function TranscriptFeed({
  harness,
  sessionId,
  title,
  onToast,
}: {
  harness: string;
  sessionId: string;
  title?: string;
  onToast: (message: string) => void;
}) {
  const { t } = useI18n();
  const s = t.sessions;
  const [turns, setTurns] = useState<TranscriptTurn[] | null>(null);
  const [query, setQuery] = useState("");
  const { copied: copiedAll, copy: copyAllText } = useClipboard(1500);

  useEffect(() => {
    let cancelled = false;
    setTurns(null);
    setQuery("");
    void getSessionTranscript(harness, sessionId)
      .then((feed) => {
        // Defense in depth: the core already refuses system scaffolding, but
        // an older payload must never paint a system prompt as a chat bubble.
        if (!cancelled) {
          setTurns(feed.filter((turn) => turn.role === "user" || turn.role === "assistant"));
        }
      })
      .catch(() => {
        if (!cancelled) setTurns([]);
      });
    return () => {
      cancelled = true;
    };
  }, [harness, sessionId]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!turns || !needle) return turns;
    return turns.filter((turn) => turn.content.toLowerCase().includes(needle));
  }, [turns, query]);

  if (turns === null || visible === null) {
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

  const filtering = query.trim().length > 0;

  const exportMarkdown = () => {
    const markdown = formatTranscriptMarkdown(turns, { harness, id: sessionId, title });
    const blob = new Blob([markdown], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${harness}-${sessionId}.md`;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-2">
      <div className="sticky top-0 z-10 space-y-1.5 bg-[#0b0b0e]/95 py-1 backdrop-blur-sm">
        <div className="flex items-center gap-1.5">
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-zinc-600" />
            <input
              type="text"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={s.searchMessages}
              aria-label={s.searchMessages}
              className="w-full rounded-lg border border-white/10 bg-black/30 py-1.5 pr-7 pl-7 text-[11.5px] text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-emerald-500/40"
            />
            {query && (
              <button
                type="button"
                onClick={() => setQuery("")}
                aria-label={t.common.close}
                className="absolute top-1/2 right-1.5 grid size-5 -translate-y-1/2 place-items-center rounded text-zinc-500 hover:text-zinc-200"
              >
                <X className="size-3" />
              </button>
            )}
          </div>
          <button
            type="button"
            title={copiedAll ? s.copiedAll : s.copyAll}
            aria-label={copiedAll ? s.copiedAll : s.copyAll}
            onClick={async () => {
              const markdown = formatTranscriptMarkdown(turns, {
                harness,
                id: sessionId,
                title,
              });
              const ok = await copyAllText(markdown);
              if (ok) onToast(s.copiedAll);
            }}
            className="grid size-7.5 shrink-0 place-items-center rounded-lg border border-white/10 bg-white/[0.02] text-zinc-400 transition hover:border-white/20 hover:text-zinc-200"
          >
            {copiedAll ? <Check className="size-3.5 text-emerald-300" /> : <Copy className="size-3.5" />}
          </button>
          <button
            type="button"
            title={s.exportMarkdown}
            aria-label={s.exportMarkdown}
            onClick={exportMarkdown}
            className="grid size-7.5 shrink-0 place-items-center rounded-lg border border-white/10 bg-white/[0.02] text-zinc-400 transition hover:border-white/20 hover:text-zinc-200"
          >
            <Download className="size-3.5" />
          </button>
        </div>
        {filtering && (
          <p role="status" className="font-mono text-[10px] text-zinc-500">
            {visible.length > 0 ? s.matchesCount(visible.length) : s.noSearchMatches}
          </p>
        )}
      </div>

      {visible.map((turn, index) => (
        <Turn key={`${index}-${turn.role}-${turn.content.length}`} turn={turn} onToast={onToast} />
      ))}
    </div>
  );
}
