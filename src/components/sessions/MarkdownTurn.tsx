import { useMemo, useState, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { cn } from "../../lib/utils";

type Block =
  | { kind: "code"; lang: string; code: string }
  | { kind: "heading"; level: 1 | 2 | 3; text: string }
  | { kind: "quote"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "paragraph"; lines: string[] };

const HEADING_SIZE: Record<1 | 2 | 3, string> = {
  1: "text-[13px]",
  2: "text-xs",
  3: "text-xs",
};

const FENCE = "```";
const HEADING = /^(#{1,3})\s+(.+)$/;
const UNORDERED = /^[-*]\s+/;
const ORDERED = /^\d+\.\s+/;
/** Inline code, bold, then italic. `_` italics avoid snake_case identifiers. */
const INLINE =
  /`([^`\n]+)`|\*\*([^*\n]+)\*\*|__([^_\n]+)__|\*([^*\n]+)\*|(?<![\w])_([^_\n]+)_(?![\w])/g;

function startsBlock(line: string): boolean {
  return (
    line.startsWith(FENCE) ||
    HEADING.test(line) ||
    line.startsWith(">") ||
    UNORDERED.test(line) ||
    ORDERED.test(line)
  );
}

/** Split chat text into the block kinds a bubble renders. */
function parseBlocks(source: string): Block[] {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const blocks: Block[] = [];
  let index = 0;

  while (index < lines.length) {
    const trimmed = lines[index].trim();
    if (!trimmed) {
      index += 1;
      continue;
    }

    if (trimmed.startsWith(FENCE)) {
      const lang = trimmed.slice(FENCE.length).trim();
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index].trim().startsWith(FENCE)) {
        code.push(lines[index]);
        index += 1;
      }
      index += 1;
      blocks.push({ kind: "code", lang, code: code.join("\n") });
      continue;
    }

    const heading = HEADING.exec(trimmed);
    if (heading) {
      blocks.push({
        kind: "heading",
        level: heading[1].length as 1 | 2 | 3,
        text: heading[2].trim(),
      });
      index += 1;
      continue;
    }

    if (trimmed.startsWith(">")) {
      const quote: string[] = [];
      while (index < lines.length && lines[index].trim().startsWith(">")) {
        quote.push(lines[index].trim().replace(/^>\s?/, ""));
        index += 1;
      }
      blocks.push({ kind: "quote", text: quote.join("\n") });
      continue;
    }

    if (UNORDERED.test(trimmed) || ORDERED.test(trimmed)) {
      const ordered = ORDERED.test(trimmed);
      const marker = ordered ? ORDERED : UNORDERED;
      const items: string[] = [];
      while (index < lines.length && marker.test(lines[index].trim())) {
        items.push(lines[index].trim().replace(marker, ""));
        index += 1;
      }
      blocks.push({ kind: "list", ordered, items });
      continue;
    }

    const paragraph: string[] = [];
    while (index < lines.length) {
      const next = lines[index];
      if (!next.trim()) break;
      if (paragraph.length > 0 && startsBlock(next.trim())) break;
      paragraph.push(next.trimEnd());
      index += 1;
    }
    blocks.push({ kind: "paragraph", lines: paragraph });
  }

  return blocks;
}

/** Inline spans: code pill, bold, italic. Plain text is returned untouched. */
function renderInline(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let cursor = 0;
  let key = 0;

  for (const match of text.matchAll(INLINE)) {
    const at = match.index ?? 0;
    if (at > cursor) nodes.push(text.slice(cursor, at));
    const [, code, bold, boldAlt, italic, italicAlt] = match;
    if (code !== undefined) {
      nodes.push(
        <code
          key={key++}
          className="rounded bg-white/[0.08] px-1 py-0.5 font-mono text-[11px] text-emerald-200/90"
        >
          {code}
        </code>,
      );
    } else if (bold !== undefined || boldAlt !== undefined) {
      nodes.push(
        <strong key={key++} className="font-semibold text-zinc-100">
          {bold ?? boldAlt}
        </strong>,
      );
    } else {
      nodes.push(
        <em key={key++} className="text-zinc-300 italic">
          {italic ?? italicAlt}
        </em>,
      );
    }
    cursor = at + match[0].length;
  }

  if (cursor < text.length) nodes.push(text.slice(cursor));
  return nodes;
}

/** Fenced block with a language badge and its own copy affordance. */
function CodeBlock({ lang, code }: { lang: string; code: string }) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);

  const copy = () => {
    void navigator.clipboard
      .writeText(code)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => undefined);
  };

  return (
    <div className="my-1.5 min-w-0 overflow-hidden rounded-lg border border-white/10 bg-black/45">
      <div className="flex items-center justify-between gap-2 border-b border-white/[0.06] px-2 py-1">
        <span className="font-mono text-[9px] font-medium tracking-[0.1em] text-zinc-500 uppercase">
          {lang || "code"}
        </span>
        <button
          type="button"
          onClick={copy}
          title={t.sessions.copyCode}
          aria-label={t.sessions.copyCode}
          className="grid size-5 place-items-center rounded text-zinc-500 transition hover:text-emerald-300"
        >
          {copied ? <Check className="size-3 text-emerald-300" /> : <Copy className="size-3" />}
        </button>
      </div>
      <pre className="overflow-x-auto px-2.5 py-2 font-mono text-[11px] leading-relaxed text-zinc-200">
        <code>{code}</code>
      </pre>
    </div>
  );
}

function ListBlock({ ordered, items }: { ordered: boolean; items: string[] }) {
  const body = items.map((item, index) => (
    <li key={index}>{renderInline(item)}</li>
  ));

  return ordered ? (
    <ol className="list-decimal space-y-0.5 pl-4">{body}</ol>
  ) : (
    <ul className="list-disc space-y-0.5 pl-4">{body}</ul>
  );
}

/**
 * Safe, dependency-free Markdown for one chat bubble. Everything is rendered
 * as React nodes (never HTML), so harness text cannot inject markup.
 */
export function MarkdownTurn({ content, role }: { content: string; role: string }) {
  const blocks = useMemo(() => parseBlocks(content), [content]);
  const isUser = role === "user";

  return (
    <div className="selectable-text space-y-1.5 font-sans text-[12px] leading-relaxed text-zinc-200">
      {blocks.map((block, index) => {
        switch (block.kind) {
          case "code":
            return <CodeBlock key={index} lang={block.lang} code={block.code} />;
          case "heading":
            return (
              <p
                key={index}
                className={cn(
                  "font-semibold",
                  HEADING_SIZE[block.level],
                  isUser ? "text-emerald-100" : "text-zinc-100",
                )}
              >
                {renderInline(block.text)}
              </p>
            );
          case "quote":
            return (
              <blockquote
                key={index}
                className="whitespace-pre-wrap border-l-2 border-emerald-500/30 pl-2 text-zinc-400 italic"
              >
                {renderInline(block.text)}
              </blockquote>
            );
          case "list":
            return (
              <ListBlock key={index} ordered={block.ordered} items={block.items} />
            );
          default:
            return (
              <p key={index} className="whitespace-pre-wrap">
                {renderInline(block.lines.join("\n"))}
              </p>
            );
        }
      })}
    </div>
  );
}
