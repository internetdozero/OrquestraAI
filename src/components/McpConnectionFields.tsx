import { useI18n } from "../hooks/useI18n";
import { formatEnv, parseEnv } from "../lib/mcps";
import { cn } from "../lib/utils";

const inputClass =
  "h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-2.5 font-mono text-[12px] text-zinc-200 placeholder:text-zinc-600 outline-none transition focus:border-emerald-500/40 focus:bg-zinc-900 focus:ring-2 focus:ring-emerald-500/10";

export function McpConnectionFields({
  url,
  headers,
  onUrlChange,
  onHeadersChange,
}: {
  url: string;
  headers: Record<string, string>;
  onUrlChange: (value: string) => void;
  onHeadersChange: (value: Record<string, string>) => void;
}) {
  const { t } = useI18n();

  return (
    <>
      <div>
        <label
          className="mb-1 block text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase"
          htmlFor="mcp-url"
        >
          URL
        </label>
        <input
          id="mcp-url"
          value={url}
          onChange={(event) => onUrlChange(event.target.value)}
          placeholder="https://example.com/mcp"
          spellCheck={false}
          className={inputClass}
        />
      </div>
      <div>
        <label
          className="mb-1 block text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase"
          htmlFor="mcp-headers"
        >
          {t.mcps.headers} <span className="text-zinc-600">· KEY=VALUE per line</span>
        </label>
        <textarea
          id="mcp-headers"
          value={formatEnv(headers)}
          onChange={(event) => onHeadersChange(parseEnv(event.target.value))}
          rows={2}
          spellCheck={false}
          placeholder="Authorization=Bearer ${API_TOKEN}"
          className={cn(inputClass, "h-auto resize-y py-2 leading-relaxed")}
        />
      </div>
    </>
  );
}
