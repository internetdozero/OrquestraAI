import { memo, useState } from "react";
import { CircleSlash, Info, Plug, Power, Trash2 } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { coversHarness, executionKind, isLockedByGlobal, type McpServer } from "../lib/mcps";
import { listingTokens, mcpCatalogTokens } from "../lib/tokens";
import { cn } from "../lib/utils";
import { OfficialBadge } from "./OfficialBadge";
import { TokenPair } from "./TokenPair";

/** Badge for the execution kind derived from the command. */
export function KindPill({ kind }: { kind: string }) {
  return (
    <span
      className={cn(
        "rounded border px-1.5 py-px font-mono text-[9.5px] font-medium tracking-wide uppercase",
        kind === "npx"
          ? "border-cyan-500/25 bg-cyan-500/10 text-cyan-300/90"
          : kind === "python"
            ? "border-amber-500/25 bg-amber-500/10 text-amber-300/90"
            : "border-white/[0.09] bg-white/[0.04] text-zinc-400",
      )}
    >
      {kind}
    </span>
  );
}

/** Projection switch for one server in one scope. */
export function McpToggle({
  checked,
  locked,
  busy,
  label,
  onChange,
}: {
  checked: boolean;
  locked: boolean;
  busy: boolean;
  label: string;
  onChange: () => void;
}) {
  const { t } = useI18n();

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={
        locked
          ? t.mcps.toggleInherited(label)
          : checked
            ? t.mcps.toggleDisable(label)
            : t.mcps.toggleEnable(label)
      }
      title={locked ? t.skills.inheritedLockTitle : undefined}
      disabled={busy || locked}
      onClick={onChange}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors duration-200 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 focus-visible:outline-none",
        checked
          ? "border-[var(--accent-ring)] bg-[var(--accent-solid)]"
          : "border-white/10 bg-zinc-800 hover:bg-zinc-700",
        busy && "cursor-wait opacity-60",
        locked && "cursor-not-allowed",
      )}
    >
      <span
        className={cn(
          "block size-3.5 rounded-full bg-white shadow-sm transition-transform duration-200",
          checked ? "translate-x-[18px]" : "translate-x-[3px]",
        )}
      />
    </button>
  );
}

/**
 * One entry in the MCP catalog, with its scope switch and card actions.
 * Memoized: the grid re-renders every keystroke while filtering, and only
 * cards whose server, scope or busy flags actually changed may re-render —
 * which requires every callback prop to hold a stable identity (see the
 * useCallback handlers in McpView and useMcpMutations).
 */
export const McpCard = memo(function McpCard({
  server,
  scope,
  scopeLabel,
  busy,
  onToggle,
  onInspect,
  onEdit,
  onDelete,
}: {
  server: McpServer;
  scope: string;
  scopeLabel: string;
  busy: boolean;
  onToggle: (server: McpServer) => void;
  onInspect: (server: McpServer) => void;
  onEdit: (server: McpServer) => void;
  onDelete: (server: McpServer) => void;
}) {
  const { t } = useI18n();
  const [confirming, setConfirming] = useState(false);

  const active = coversHarness(server, scope);
  const locked = isLockedByGlobal(server, scope);
  const kind = executionKind(server.command);
  const anyActive = server.globalEnabled || server.enabledHarnesses.length > 0;

  return (
    <article
      data-card-id={server.id}
      aria-busy={busy}
      className={cn(
        "group relative flex flex-col gap-3 overflow-hidden rounded-xl border p-4",
        "transition-[border-color,background-color,box-shadow] duration-150 ease-out",
        // Same reason as SkillCard: no geometry shift on hover, and keep the
        // repaint local to this card while the grid scrolls.
        "hover:shadow-[0_8px_30px_-12px_rgba(0,0,0,0.9)]",
        "transform-gpu [backface-visibility:hidden] [contain:content]",
        // Stretch to fill the row's tallest item like every other card in the grid.
        "h-full",
        active
          ? "accent-border bg-gradient-to-b from-[var(--accent-bg)] to-zinc-900/40 hover:border-[var(--accent-ring)]"
          : "border-white/[0.07] bg-zinc-900/30 hover:border-white/15 hover:bg-zinc-900/50",
        busy && "opacity-80",
      )}
    >
      {active && (
        <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-[var(--accent-glow)] to-transparent" />
      )}

      <div className="flex items-start gap-3">
        <div
          className={cn(
            "grid size-9 shrink-0 place-items-center rounded-lg border transition-colors",
            active
              ? "accent-border accent-bg-soft accent-text"
              : "border-white/[0.07] bg-white/[0.03] text-zinc-500",
          )}
        >
          <Plug className="size-4" />
        </div>

        <div className="min-w-0 flex-1">
          <h3
            className="truncate font-mono text-[13px] font-medium text-zinc-100"
            title={server.name}
          >
            {server.name}
          </h3>
          <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
            <KindPill kind={kind} />
            {server.official === true && <OfficialBadge />}
            {locked ? (
              <span className="accent-text text-[9.5px] font-medium opacity-90">
                {t.skills.inheritedFromGlobal}
              </span>
            ) : active ? (
              <span className="accent-text flex items-center gap-1 text-[10px] font-medium">
                <span className="accent-dot size-1 rounded-full" />
                {t.skills.activePill}
              </span>
            ) : (
              <span className="flex items-center gap-1 text-[10px] text-zinc-600">
                <span className="size-1 rounded-full bg-zinc-600" />
                {t.mcps.inactive}
              </span>
            )}
            {anyActive && !active && (
              <span className="font-mono text-[9.5px] text-zinc-600">{t.mcps.onElsewhere}</span>
            )}
          </div>
        </div>

        <McpToggle
          checked={active}
          locked={locked}
          busy={busy}
          label={`${server.name} in ${scopeLabel}`}
          onChange={() => onToggle(server)}
        />
      </div>

      <p className="line-clamp-2 min-h-[2.1rem] text-[12.5px] leading-relaxed text-zinc-400">
        {server.description || <span className="text-zinc-600 italic">{t.mcps.noDescription}</span>}
      </p>

      <TokenPair
        entry={listingTokens(server.name, server.description)}
        full={mcpCatalogTokens(server)}
        fullHint={t.tokens.fullMcpHint}
      />

      <button
        type="button"
        onClick={() => onInspect(server)}
        className="flex items-center gap-1.5 self-start rounded text-[10.5px] font-medium text-zinc-500 transition hover:text-zinc-300 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
      >
        <Info className="size-3" />
        {t.mcps.inspect}
      </button>

      <div className="mt-auto flex items-center justify-between gap-2 border-t border-white/[0.05] pt-3">
        {confirming ? (
          <div className="flex w-full flex-wrap items-center justify-between gap-2">
            <span className="min-w-0 flex-1 text-[10.5px] leading-snug text-amber-200/90">
              {t.mcps.deleteConfirm(server.name)}
            </span>
            <span className="flex shrink-0 items-center gap-1">
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
            </span>
          </div>
        ) : (
          <>
            <div className="flex min-w-0 items-center gap-1.5">
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
            </div>
            <span
              className={cn(
                "flex items-center gap-1.5 font-mono text-[10px]",
                active ? "accent-text opacity-80" : "text-zinc-600",
              )}
            >
              {locked ? (
                <Plug className="size-3" />
              ) : active ? (
                <Power className="size-3" />
              ) : (
                <CircleSlash className="size-3" />
              )}
              {locked
                ? t.mcps.inheritedShort
                : active
                  ? t.mcps.inScope(scopeLabel)
                  : t.skills.notProjected}
            </span>
          </>
        )}
      </div>
    </article>
  );
});
