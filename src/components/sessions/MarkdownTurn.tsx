import { useMemo, useState, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { cn } from "../../lib/utils";

type Block =
  | { kind: "code"; lang: string; code: string }
  | { kind: "heading"; level: 1 | 2 | 3 | 4 | 5 | 6; text: string }
  | { kind: "table"; headers: string[]; rows: string[][] }
  | { kind: "quote"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "divider" }
  | { kind: "paragraph"; lines: string[] };

const HEADING_SIZE: Record<1 | 2 | 3 | 4 | 5 | 6, string> = {
  1: "text-[15px] font-bold tracking-tight text-zinc-100 mt-2 mb-1",
  2: "text-[13.5px] font-semibold text-zinc-100 mt-2 mb-0.5",
  3: "text-[12.5px] font-semibold text-zinc-200 mt-1.5 mb-0.5",
  4: "text-[12px] font-medium text-zinc-300 mt-1",
  5: "text-[11.5px] font-medium text-zinc-400 mt-1",
  6: "text-[11px] font-medium text-zinc-500 uppercase tracking-wider mt-1",
};

const FENCE = "```";
const HEADING = /^(#{1,6})\s+(.+)$/;
const SEPARATOR_CELL = /^:?-{1,}:?$/;
const UNORDERED = /^[-*]\s+/;
const ORDERED = /^\d+\.\s+/;
const HORIZONTAL_RULE = /^(?:-{3,}|\*{3,}|_{3,})$/;
/** Links first (`[`code`](url)` keeps its badge), then code, bold, italic. */
const INLINE =
  /\[([^\]\n]+)\]\(([^)\s]+)\)|`([^`\n]+)`|\*\*([^*\n]+)\*\*|__([^_\n]+)__|\*([^*\n]+)\*|(?<![\w])_([^_\n]+)_(?![\w])/g;

/** Only navigable schemes (plus relative paths) become anchors; notably
 *  `javascript:` renders as inert text instead of a clickable link. */
function isSafeUrl(url: string): boolean {
  if (/^(#|\/|\.\.?\/)/.test(url)) return true;
  const scheme = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.exec(url)?.[0].toLowerCase();
  return scheme === "http:" || scheme === "https:" || scheme === "file:" || scheme === "mailto:";
}

function startsBlock(line: string): boolean {
  return (
    line.startsWith(FENCE) ||
    HEADING.test(line) ||
    HORIZONTAL_RULE.test(line) ||
    line.startsWith(">") ||
    UNORDERED.test(line) ||
    ORDERED.test(line)
  );
}

/** `| a | b |` → ["a", "b"]; outer pipes leave empty edge cells to drop. */
function splitCells(line: string): string[] {
  const cells = line.split("|").map((cell) => cell.trim());
  if (cells.length > 0 && cells[0] === "") cells.shift();
  if (cells.length > 0 && cells[cells.length - 1] === "") cells.pop();
  return cells;
}

/** A header row starts a GFM table only when a `| --- |` row follows it. */
function startsTable(lines: string[], index: number): boolean {
  const header = lines[index]?.trim() ?? "";
  const separator = lines[index + 1]?.trim() ?? "";
  if (!header.includes("|") || splitCells(header).length === 0) return false;
  const parts = splitCells(separator);
  return parts.length > 0 && parts.every((cell) => SEPARATOR_CELL.test(cell));
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
        level: heading[1].length as 1 | 2 | 3 | 4 | 5 | 6,
        text: heading[2].trim(),
      });
      index += 1;
      continue;
    }

    if (startsTable(lines, index)) {
      const headers = splitCells(trimmed);
      const rows: string[][] = [];
      index += 2;
      while (index < lines.length) {
        const next = lines[index].trim();
        if (!next || !next.includes("|")) break;
        rows.push(splitCells(next));
        index += 1;
      }
      blocks.push({ kind: "table", headers, rows });
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

    if (HORIZONTAL_RULE.test(trimmed)) {
      blocks.push({ kind: "divider" });
      index += 1;
      continue;
    }

    const paragraph: string[] = [];
    while (index < lines.length) {
      const next = lines[index];
      if (!next.trim()) break;
      // A pipe row only breaks the paragraph when a separator row follows;
      // otherwise it is plain text (e.g. `a | b` without a table).
      if (startsTable(lines, index)) break;
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
    const [, linkLabel, linkUrl, code, bold, boldAlt, italic, italicAlt] = match;
    if (linkLabel !== undefined && linkUrl !== undefined) {
      if (!isSafeUrl(linkUrl)) nodes.push(match[0]);
      else {
        nodes.push(
          <a
            key={key++}
            href={linkUrl}
            target="_blank"
            rel="noopener noreferrer"
            onClick={() => {
              if (linkUrl.startsWith("file://")) window.open(linkUrl, "_blank");
            }}
            className="font-medium break-all text-emerald-400 underline decoration-emerald-500/40 underline-offset-2 transition hover:text-emerald-300 hover:decoration-emerald-400"
          >
            {renderInline(linkLabel)}
          </a>,
        );
      }
    } else if (code !== undefined) {
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

function TableBlock({ headers, rows }: { headers: string[]; rows: string[][] }) {
  return (
    <div className="my-2 max-w-full overflow-x-auto rounded-lg border border-white/10 bg-black/20">
      <table className="w-full border-collapse text-left text-[11.5px]">
        {headers.length > 0 && (
          <thead className="border-b border-white/10 bg-white/[0.03]">
            <tr>
              {headers.map((cell, idx) => (
                <th key={idx} className="px-3 py-2 font-semibold text-zinc-200">
                  {renderInline(cell)}
                </th>
              ))}
            </tr>
          </thead>
        )}
        <tbody className="divide-y divide-white/[0.04]">
          {rows.map((row, rIdx) => (
            <tr key={rIdx} className="transition hover:bg-white/[0.02]">
              {row.map((cell, cIdx) => (
                <td key={cIdx} className="px-3 py-1.5 text-zinc-300">
                  {renderInline(cell)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ListBlock({ ordered, items }: { ordered: boolean; items: string[] }) {
  const body = items.map((item, index) => (
    <li key={index} className="break-words [overflow-wrap:anywhere]">
      {renderInline(item)}
    </li>
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
    <div className="selectable-text min-w-0 space-y-1.5 font-sans text-[12px] leading-relaxed break-words text-zinc-200 [overflow-wrap:anywhere]">
      {blocks.map((block, index) => {
        switch (block.kind) {
          case "code":
            return <CodeBlock key={index} lang={block.lang} code={block.code} />;
          case "heading":
            return (
              <p
                key={index}
                className={cn(HEADING_SIZE[block.level], isUser && "text-emerald-100")}
              >
                {renderInline(block.text)}
              </p>
            );
          case "table":
            return <TableBlock key={index} headers={block.headers} rows={block.rows} />;
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
            return <ListBlock key={index} ordered={block.ordered} items={block.items} />;
          case "divider":
            return <hr key={index} className="my-2 border-0 border-t border-white/10" />;
          default:
            return (
              <p key={index} className="whitespace-pre-wrap break-words [overflow-wrap:anywhere]">
                {renderInline(block.lines.join("\n"))}
              </p>
            );
        }
      })}
    </div>
  );
}
