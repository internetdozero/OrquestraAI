import { useMemo, useRef } from "react";
import type { KeyboardEvent } from "react";
import { FileText, Link2, Loader2, Save } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { HARNESSES } from "../lib/harnesses";
import { estimateTokens } from "../lib/tokens";
import { cn, formatTokens } from "../lib/utils";

interface AgentsEditorProps {
  content: string;
  savedContent: string;
  loading: boolean;
  saving: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
}

export function AgentsEditor({
  content,
  savedContent,
  loading,
  saving,
  onChange,
  onSave,
}: AgentsEditorProps) {
  const { t } = useI18n();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const gutterRef = useRef<HTMLDivElement>(null);
  const lineCount = useMemo(() => Math.max(1, content.split("\n").length), [content]);
  const dirty = content !== savedContent;

  const handleScroll = () => {
    if (gutterRef.current && textareaRef.current) {
      gutterRef.current.scrollTop = textareaRef.current.scrollTop;
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Tab") {
      event.preventDefault();
      const target = event.currentTarget;
      const start = target.selectionStart;
      const end = target.selectionEnd;
      const next = `${content.slice(0, start)}  ${content.slice(end)}`;
      onChange(next);

      window.requestAnimationFrame(() => {
        target.selectionStart = start + 2;
        target.selectionEnd = start + 2;
      });
      return;
    }

    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      onSave();
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex items-start gap-3">
          <div className="grid size-10 place-items-center rounded-xl border border-emerald-500/20 bg-emerald-500/10">
            <FileText className="size-4 text-emerald-300" />
          </div>
          <div>
            <p className="text-[10px] font-medium tracking-[0.14em] text-zinc-500 uppercase">
              {t.agents.configuring}
            </p>
            <h2 className="text-base font-semibold tracking-tight text-zinc-100">AGENTS.md</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">
              {t.agents.masterHint(HARNESSES.length)}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <span
            className={cn(
              "flex items-center gap-1.5 text-[11px]",
              dirty ? "text-amber-300/90" : "text-zinc-500",
            )}
          >
            <span
              className={cn("size-1.5 rounded-full", dirty ? "bg-amber-400" : "bg-emerald-400")}
            />
            {dirty ? t.agents.unsaved : t.agents.saved}
          </span>
          <button
            type="button"
            onClick={onSave}
            disabled={!dirty || loading || saving}
            className="flex items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-1.5 text-[11.5px] font-medium text-emerald-200 transition hover:border-emerald-400/50 hover:bg-emerald-500/15 disabled:cursor-not-allowed disabled:border-white/10 disabled:bg-white/[0.03] disabled:text-zinc-500 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
          >
            {saving ? <Loader2 className="size-3.5 animate-spin" /> : <Save className="size-3.5" />}
            {t.agents.saveDeploy}
          </button>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-white/[0.07] bg-[#0b0b0e] shadow-[0_12px_40px_-24px_rgba(0,0,0,1)]">
        <div className="flex items-center justify-between border-b border-white/[0.05] bg-white/[0.015] px-3 py-2">
          <div className="flex min-w-0 items-center gap-2 font-mono text-[11px] text-zinc-400">
            <span className="size-2 shrink-0 rounded-full bg-emerald-400/80" />
            <span className="truncate">~/.orquestra/AGENTS.md</span>
          </div>
          <div className="flex shrink-0 items-center gap-3 font-mono text-[10px] text-zinc-600">
            <span>{t.agents.lineCount(lineCount)}</span>
            <span className="hidden sm:inline">{t.agents.charCount(content.length)}</span>
            <span title={t.tokens.agentsHint}>
              ≈ {formatTokens(estimateTokens(content))} {t.tokens.unit}
            </span>
          </div>
        </div>

        <div className="relative flex min-h-0 flex-1 overflow-hidden">
          <div
            ref={gutterRef}
            aria-hidden="true"
            className="selectable-text w-12 shrink-0 overflow-hidden border-r border-white/[0.04] bg-white/[0.015] py-3 pr-2 text-right font-mono text-[12.5px] leading-5 text-zinc-600 tabular-nums"
          >
            {Array.from({ length: lineCount }, (_, index) => (
              <div key={index}>{index + 1}</div>
            ))}
          </div>

          <textarea
            ref={textareaRef}
            value={content}
            onChange={(event) => onChange(event.target.value)}
            onScroll={handleScroll}
            onKeyDown={handleKeyDown}
            wrap="off"
            spellCheck={false}
            disabled={loading}
            aria-label={t.agents.editorAria}
            placeholder={loading ? t.agents.loadingPlaceholder : t.agents.emptyPlaceholder}
            className="selectable-text min-h-0 flex-1 resize-none bg-transparent py-3 pr-4 pl-3 font-mono text-[12.5px] leading-5 text-zinc-200 caret-emerald-400 outline-none placeholder:text-zinc-600 disabled:cursor-wait"
          />

          {loading && (
            <div className="absolute inset-0 flex items-center justify-center gap-2 bg-[#0b0b0e]/70 text-[11px] text-zinc-400">
              <Loader2 className="size-3.5 animate-spin text-emerald-400" />
              {t.agents.reading}
            </div>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-2 border-t border-white/[0.05] px-3 py-2">
          <span className="flex items-center gap-1 text-[9.5px] font-medium tracking-[0.12em] text-zinc-600 uppercase">
            <Link2 className="size-3" />
            {t.agents.linkedHarnesses}
          </span>
          {HARNESSES.map((harness) => (
            <span
              key={harness.id}
              className="rounded border border-white/[0.07] bg-white/[0.03] px-1.5 py-0.5 text-[10px] text-zinc-400"
            >
              {harness.label}
            </span>
          ))}
          <span className="ml-auto font-mono text-[9.5px] text-zinc-600">
            {t.agents.masterFile}
          </span>
        </div>
      </div>
    </div>
  );
}
