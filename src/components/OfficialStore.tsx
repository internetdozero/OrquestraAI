import { useEffect, useState } from "react";
import {
  Download,
  ExternalLink,
  Loader2,
  Plus,
  Power,
  ShieldCheck,
} from "lucide-react";
import {
  listOfficialStore,
  openExternalUrl,
  type OfficialMcpEntry,
  type OfficialSkillEntry,
} from "../lib/tauri";
import { useI18n } from "../hooks/useI18n";
import { cn } from "../lib/utils";

export type StoreStatus = "active" | "idle" | "missing";
export type StoreActionKind = "install" | "enable" | "add";

export interface StoreRowAction {
  kind: StoreActionKind;
  run: () => Promise<void>;
}

type StoreEntry = OfficialSkillEntry | OfficialMcpEntry;

interface OfficialStoreProps {
  kind: "skill" | "mcp";
  /** Live install state per id, read from the parent's catalog data. */
  statusOf: (id: string) => StoreStatus;
  /** Primary row action, or null when the entry has none. */
  actionFor: (entry: StoreEntry, status: StoreStatus) => StoreRowAction | null;
}

const ACTION_ICON = {
  install: Download,
  enable: Power,
  add: Plus,
} as const;

/**
 * Curated Personal Store panel: the official entries fetched from the Rust
 * core, each with its live install state and a one-click primary action.
 * Row spinners are tracked here; the parent refreshes its catalog data
 * inside each action so `statusOf` re-evaluates on the next render.
 */
export function OfficialStore({ kind, statusOf, actionFor }: OfficialStoreProps) {
  const { t } = useI18n();
  const s = t.store;
  const [entries, setEntries] = useState<StoreEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listOfficialStore()
      .then((store) => {
        if (!cancelled) setEntries(kind === "skill" ? store.skills : store.mcps);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(
            s.loadFailed(
              cause instanceof Error ? cause.message : t.common.unexpected,
            ),
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, [kind, s, t.common.unexpected]);

  if (error) {
    return (
      <div role="alert" className="rounded-xl border border-red-500/25 bg-red-500/[0.07] px-4 py-3 text-[12px] text-red-200">
        {error}
      </div>
    );
  }

  if (!entries) {
    return (
      <div className="flex items-center justify-center gap-2 py-16 text-[12px] text-zinc-500">
        <Loader2 className="size-4 animate-spin" />
        {s.loading}
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-xl border border-amber-500/[0.14] bg-black/20">
      <div className="flex items-center gap-2 border-b border-amber-500/10 px-4 py-3">
        <ShieldCheck className="size-4 shrink-0 text-amber-300" />
        <div className="min-w-0">
          <h3 className="text-[13px] font-semibold text-zinc-100">{s.title}</h3>
          <p className="truncate text-[11px] text-zinc-500">{s.subtitle}</p>
        </div>
      </div>

      <ol className="divide-y divide-white/[0.04]">
        {entries.map((entry) => {
          const status = statusOf(entry.id);
          const action = actionFor(entry, status);
          const ActionIcon = action ? ACTION_ICON[action.kind] : null;
          const meta =
            kind === "skill"
              ? (entry as OfficialSkillEntry).category
              : [ (entry as OfficialMcpEntry).command, ...(entry as OfficialMcpEntry).args ].join(" ");
          const busy = busyId === entry.id;

          return (
            <li key={entry.id} className="flex items-start gap-3 px-4 py-3 transition hover:bg-white/[0.02]">
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <span className="truncate font-mono text-[12.5px] font-medium text-zinc-100">
                    {entry.name}
                  </span>
                  {status === "missing" ? (
                    <span className="text-[10px] text-zinc-600">{t.common.notInstalled}</span>
                  ) : (
                    <span className="text-[10px] font-medium text-emerald-400">{s.installed}</span>
                  )}
                </div>
                <p className="mt-0.5 line-clamp-2 text-[12px] leading-relaxed text-zinc-400">
                  {entry.description}
                </p>
                <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[10px] text-zinc-600">
                  <span className="rounded border border-white/[0.07] bg-white/[0.03] px-1.5 py-px">
                    {meta}
                  </span>
                  {entry.repoUrl ? (
                    <button
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        void openExternalUrl(entry.repoUrl as string);
                      }}
                      className="flex items-center gap-1 text-zinc-500 transition hover:text-zinc-300"
                    >
                      <ExternalLink className="size-2.5" />
                      {s.viewRepo}
                    </button>
                  ) : (
                    <span className="italic">{s.unpublished}</span>
                  )}
                </div>
              </div>

              {action && ActionIcon && (
                <button
                  type="button"
                  disabled={busy}
                  onClick={async () => {
                    setBusyId(entry.id);
                    setError(null);
                    try {
                      await action.run();
                    } catch (cause: unknown) {
                      setError(
                        s.actionFailed(
                          cause instanceof Error ? cause.message : t.common.unexpected,
                        ),
                      );
                    } finally {
                      setBusyId(null);
                    }
                  }}
                  className={cn(
                    "flex shrink-0 items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-[11px] font-medium transition disabled:cursor-wait disabled:opacity-60",
                    action.kind === "enable"
                      ? "border-emerald-500/30 bg-emerald-500/15 text-emerald-200 hover:bg-emerald-500/25"
                      : "border-amber-500/30 bg-amber-500/15 text-amber-200 hover:bg-amber-500/25",
                  )}
                >
                  {busy ? (
                    <Loader2 className="size-3.5 animate-spin" />
                  ) : (
                    <ActionIcon className="size-3.5" />
                  )}
                  {action.kind === "install" ? s.install : action.kind === "enable" ? s.enable : s.add}
                </button>
              )}
            </li>
          );
        })}
      </ol>
    </div>
  );
}
