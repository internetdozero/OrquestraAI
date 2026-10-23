import { memo, useState } from "react";
import { Info, Plug, Trash2 } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { coversHarness, executionKind, isLockedByGlobal, type McpServer } from "../lib/mcps";
import { listingTokens } from "../lib/tokens";
import { formatTokens } from "../lib/utils";
import { KindPill, McpToggle } from "./McpCard";
import { OfficialBadge } from "./OfficialBadge";

interface McpListRowProps {
  server: McpServer;
  scope: string;
  scopeLabel: string;
  busy: boolean;
  onToggle: (server: McpServer) => void;
  onInspect: (server: McpServer) => void;
  onEdit: (server: McpServer) => void;
  onDelete: (server: McpServer) => void;
}

/**
 * Single-line summary for the MCP list view; same toggle / inspect /
 * edit / delete actions as the card, including the inline delete confirm.
 */
export const McpListRow = memo(function McpListRow({
  server,
  scope,
  scopeLabel,
  busy,
  onToggle,
  onInspect,
  onEdit,
  onDelete,
}: McpListRowProps) {
  const { t } = useI18n();
  const [confirming, setConfirming] = useState(false);

  const active = coversHarness(server, scope);
  const locked = isLockedByGlobal(server, scope);
  const kind = executionKind(server.command);

  return (
    <div
      data-card-id={server.id}
      aria-busy={busy}
      className="flex items-center gap-2.5 rounded-xl border border-white/[0.07] bg-zinc-900/60 px-3 py-2.5"
    >
      <span className="grid size-8 shrink-0 place-items-center rounded-lg border border-white/[0.07] bg-white/[0.03] text-zinc-500">
        <Plug className="size-4" />
      </span>
      <div className="w-44 min-w-0 shrink-0">
        <h3 className="truncate font-mono text-[13px] font-medium text-zinc-100" title={server.name}>
          {server.name}
        </h3>
        <div className="mt-0.5 flex items-center gap-1.5">
          <KindPill kind={kind} />
          {server.official === true && <OfficialBadge />}
        </div>
      </div>
      <p className="hidden min-w-0 flex-1 truncate text-[12px] text-zinc-400 lg:block">
        {server.description || <span className="text-zinc-600 italic">{t.mcps.noDescription}</span>}
      </p>
      <span
        className="hidden shrink-0 font-mono text-[10.5px] text-zinc-500 md:inline"
        title={t.tokens.fullMcpHint}
      >
        ≈ {formatTokens(listingTokens(server.name, server.description))}
      </span>
      {locked ? (
        <span className="accent-text shrink-0 text-[10px] font-medium">
          {t.skills.inheritedFromGlobal}
        </span>
      ) : (
        <span
          className={`flex shrink-0 items-center gap-1 text-[10px] font-medium ${active ? "accent-text" : "text-zinc-600"}`}
        >
          <span className={`size-1 rounded-full ${active ? "accent-dot" : "bg-zinc-600"}`} />
          {active ? t.skills.activePill : t.mcps.inactive}
        </span>
      )}
      {confirming ? (
        <div className="flex shrink-0 items-center gap-1.5">
          <span className="max-w-48 truncate text-[10.5px] text-amber-200/90">
            {t.mcps.deleteConfirm(server.name)}
          </span>
          <button
            type="button"
            onClick={() => setConfirming(false)}
            disabled={busy}
            className="rounded border border-white/10 px-2 py-1 text-[10.5px] text-zinc-400 transition hover:text-zinc-200 disabled:opacity-50"
          >
            {t.presets.cancel}
          </button>
          <button
            type="button"
            onClick={() => onDelete(server)}
            disabled={busy}
            className="rounded border border-red-500/30 bg-red-500/15 px-2 py-1 text-[10.5px] font-medium text-red-200 transition hover:bg-red-500/25 disabled:cursor-wait disabled:opacity-60"
          >
            {t.common.delete}
          </button>
        </div>
      ) : (
        <div className="flex shrink-0 items-center gap-1">
          <button
            type="button"
            onClick={() => onInspect(server)}
            title={t.mcps.inspect}
            className="rounded-md p-1.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200"
          >
            <Info className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onEdit(server)}
            disabled={busy}
            className="rounded px-1.5 py-1 text-[10.5px] font-medium text-zinc-500 transition hover:bg-white/[0.04] hover:text-zinc-200 disabled:opacity-50"
          >
            {t.common.edit}
          </button>
          <button
            type="button"
            onClick={() => setConfirming(true)}
            disabled={busy}
            aria-label={t.mcps.deleteAria(server.name)}
            className="grid size-6 place-items-center rounded text-zinc-600 transition hover:bg-red-500/10 hover:text-red-300 disabled:opacity-50"
          >
            <Trash2 className="size-3" />
          </button>
          <McpToggle
            checked={active}
            locked={locked}
            busy={busy}
            label={`${server.name} in ${scopeLabel}`}
            onChange={() => onToggle(server)}
          />
        </div>
      )}
    </div>
  );
});
