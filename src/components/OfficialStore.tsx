import { useEffect, useMemo, useState } from "react";
import { Loader2, ShieldCheck, X } from "lucide-react";
import type { OfficialMcpEntry, OfficialSkillEntry } from "../lib/tauri";
import { ensureStoreCatalog, readStoreCache } from "../lib/storeCache";
import { useI18n } from "../hooks/useI18n";
import { StoreCard } from "./StoreCard";
import { StoreFilters, type StoreTierFilter } from "./StoreFilters";
import { Download, ExternalLink } from "lucide-react";
import { installRtk, openExternalUrl } from "../lib/tauri";
import { ensureRtkStatus, refreshRtkStatus } from "../lib/rtkCache";

export type StoreStatus = "active" | "idle" | "missing";
export type StoreActionKind = "install" | "enable" | "add";

export interface StoreRowAction {
  kind: StoreActionKind;
  run: () => Promise<void>;
}

export type StoreEntry = OfficialSkillEntry | OfficialMcpEntry;

interface OfficialStoreProps {
  kind: "skill" | "mcp";
  /** Live install state; receives the entry so legacy ids match too. */
  statusOf: (entry: StoreEntry) => StoreStatus;
  actionFor: (entry: StoreEntry, status: StoreStatus) => StoreRowAction | null;
  /** Current scope label for the install-and-project hint. */
  projectScope?: string;
  onToast?: (message: string) => void;
}

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string" && cause.trim()) return cause;
  if (cause instanceof Error && cause.message.trim()) return cause.message;
  if (cause) return String(cause);
  return fallback;
}

/**
 * Community & Official Hub: curated entries with tier/category filters,
 * security badges (via StoreCard) and one-click install-and-project.
 * Row spinners live here; the parent refreshes catalog data inside each
 * action so `statusOf` re-evaluates on the next render.
 */
export function OfficialStore({
  kind,
  statusOf,
  actionFor,
  projectScope,
  onToast,
}: OfficialStoreProps) {
  const { t } = useI18n();
  const s = t.store;
  // A warm cache paints immediately; the store unmounts on every tab toggle,
  // so a cold start here is the flash. Revalidate silently underneath.
  const [entries, setEntries] = useState<StoreEntry[] | null>(() => {
    const cached = readStoreCache();
    if (!cached) return null;
    return kind === "skill" ? cached.skills : cached.mcps;
  });
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [tier, setTier] = useState<StoreTierFilter>("all");
  const [category, setCategory] = useState<string>("All");
  const [rtkBusy, setRtkBusy] = useState(false);
  const [rtkStep, setRtkStep] = useState<"binary" | "skill">("binary");
  const [rtkInstalled, setRtkInstalled] = useState<string | null | undefined>(undefined);
  const [rtkMessage, setRtkMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    // Cached: mounting this tab used to spawn `rtk --version` every time.
    ensureRtkStatus()
      .then((status) => {
        if (!cancelled) setRtkInstalled(status);
      })
      .catch(() => {
        if (!cancelled) setRtkInstalled(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    ensureStoreCatalog()
      .then((store) => {
        if (!cancelled) setEntries(kind === "skill" ? store.skills : store.mcps);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setLoadError(s.loadFailed(toMessage(cause, t.common.unexpected)));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [kind, s, t.common.unexpected]);

  const categories = useMemo(() => {
    if (!entries || kind !== "skill") return [];
    const seen = new Set<string>();
    for (const entry of entries as OfficialSkillEntry[]) seen.add(entry.category);
    return [...seen].sort((a, b) => a.localeCompare(b));
  }, [entries, kind]);

  const visible = useMemo(() => {
    if (!entries) return [];
    return entries.filter((entry) => {
      const skill = entry as OfficialSkillEntry;
      const entryTier = skill.tier ?? (skill.official === false ? "community" : "official");
      if (kind === "skill" && tier !== "all" && entryTier !== tier) return false;
      if (kind === "skill" && category !== "All" && skill.category !== category) return false;
      return true;
    });
  }, [entries, kind, tier, category]);

  const rtkSkillEntry =
    kind === "skill"
      ? (entries?.find((entry) => entry.id === "rtk" && !("command" in entry)) as
          OfficialSkillEntry | undefined)
      : undefined;
  const rtkSkillStatus = rtkSkillEntry ? statusOf(rtkSkillEntry) : null;

  if (loadError) {
    return (
      <div
        role="alert"
        className="rounded-xl border border-red-500/25 bg-red-500/[0.07] px-4 py-3 text-[12px] text-red-200"
      >
        {loadError}
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

  const scopeHint = projectScope ? s.projectHint(projectScope) : undefined;

  return (
    <div className="overflow-hidden rounded-xl border border-amber-500/[0.14] bg-black/20">
      <div className="flex items-center gap-2 border-b border-amber-500/10 px-4 py-3">
        <ShieldCheck className="size-4 shrink-0 text-amber-300" />
        <div className="min-w-0">
          <h3 className="text-[13px] font-semibold text-zinc-100">{s.title}</h3>
          <p className="truncate text-[11px] text-zinc-500">
            {tier === "community" ? s.communitySubtitle : s.subtitle}
          </p>
        </div>
      </div>

      {kind === "skill" && (
        <div className="border-b border-white/[0.05]">
          <StoreFilters
            tier={tier}
            onTierChange={setTier}
            categories={["All", ...categories]}
            category={category}
            onCategoryChange={setCategory}
            labels={{
              all: s.filterAll,
              official: s.filterOfficial,
              community: s.filterCommunity,
            }}
          />
        </div>
      )}

      {actionError && (
        <div
          role="alert"
          className="mx-4 my-3 flex items-center justify-between gap-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-[12px] text-red-200"
        >
          <span className="min-w-0 flex-1 break-words">{actionError}</span>
          <button
            type="button"
            onClick={() => setActionError(null)}
            className="shrink-0 rounded p-1 text-red-300 hover:bg-red-500/20 hover:text-white"
            aria-label="Dismiss"
          >
            <X className="size-3.5" />
          </button>
        </div>
      )}

      {kind === "skill" && (
        <section className="mx-4 my-3 flex items-center gap-3 rounded-lg border border-sky-500/20 bg-sky-500/[0.04] p-3">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2 text-[12px] font-medium text-zinc-100">
              <span>{s.rtkName}</span>
              <span className="rounded-full border border-sky-500/30 bg-sky-500/10 px-1.5 py-px text-[10px] text-sky-300">
                {s.recommendedUnofficial}
              </span>
            </div>
            <p className="mt-0.5 text-[11px] text-zinc-400">{s.rtkDescription}</p>
            <button
              type="button"
              onClick={() => void openExternalUrl("https://github.com/rtk-ai/rtk")}
              className="mt-1 flex items-center gap-1 text-[10px] text-zinc-500 hover:text-zinc-300"
            >
              <ExternalLink className="size-2.5" /> {s.viewRepo}
            </button>
            {rtkBusy ? (
              <div role="status" className="mt-2 flex items-center gap-2 text-[11px] text-sky-200">
                <Loader2 className="size-3 animate-spin" />
                <span>{rtkStep === "skill" ? s.rtkSkillInstallStarted : s.rtkInstallStarted}</span>
                <span className="h-1 flex-1 overflow-hidden rounded-full bg-white/10">
                  <span className="block h-full w-1/3 animate-pulse rounded-full bg-sky-400" />
                </span>
              </div>
            ) : rtkInstalled ? (
              <p role="status" className="mt-1 text-[10px] text-emerald-300">
                {s.rtkInstalled(rtkInstalled)}
              </p>
            ) : rtkMessage ? (
              <p role="status" className="mt-1 text-[10px] text-emerald-300">
                {rtkMessage}
              </p>
            ) : null}
          </div>
          {rtkInstalled && rtkSkillStatus !== "missing" ? (
            <span className="flex shrink-0 items-center gap-1.5 rounded-lg border border-emerald-500/25 bg-emerald-500/10 px-2.5 py-1.5 text-[11px] font-medium text-emerald-200">
              <ShieldCheck className="size-3.5" /> {s.installed}
            </span>
          ) : (
            <button
              type="button"
              disabled={rtkBusy || rtkInstalled === undefined || busyId !== null}
              onClick={async () => {
                const installBinary = !rtkInstalled;
                setRtkBusy(true);
                setRtkStep(installBinary ? "binary" : "skill");
                setRtkMessage(installBinary ? s.rtkInstallStarted : s.rtkSkillInstallStarted);
                onToast?.(installBinary ? s.rtkInstallStarted : s.rtkSkillInstallStarted);
                setActionError(null);
                try {
                  if (!rtkInstalled) {
                    const result = await installRtk();
                    const status = await refreshRtkStatus();
                    setRtkInstalled(status ?? result);
                    setRtkMessage(result);
                  }
                  if (rtkSkillEntry && rtkSkillStatus === "missing") {
                    setRtkStep("skill");
                    setRtkMessage(s.rtkSkillInstallStarted);
                    const skillAction = actionFor(rtkSkillEntry, "missing");
                    if (!skillAction) throw new Error(s.rtkSkillUnavailable);
                    await skillAction.run();
                    onToast?.(s.rtkSkillInstalled);
                  } else {
                    onToast?.(s.rtkName + " instalado.");
                  }
                } catch (cause: unknown) {
                  if (!rtkInstalled) setRtkMessage(null);
                  setActionError(s.actionFailed(toMessage(cause, t.common.unexpected)));
                } finally {
                  setRtkBusy(false);
                }
              }}
              className="flex shrink-0 items-center gap-1.5 rounded-lg border border-sky-500/30 bg-sky-500/15 px-2.5 py-1.5 text-[11px] font-medium text-sky-200 transition disabled:cursor-wait disabled:opacity-60"
            >
              {rtkBusy ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Download className="size-3.5" />
              )}
              {rtkBusy
                ? rtkStep === "skill"
                  ? s.installingRtkSkill
                  : s.installingRtk
                : rtkInstalled === undefined
                  ? s.rtkChecking
                  : rtkInstalled && rtkSkillStatus === "missing"
                    ? s.installRtkSkill
                    : s.installRtk}
            </button>
          )}
        </section>
      )}

      <ol className="divide-y divide-white/[0.04]">
        {visible.map((entry) => {
          const status = statusOf(entry);
          const action = actionFor(entry, status);
          const meta =
            kind === "skill"
              ? (entry as OfficialSkillEntry).category
              : [(entry as OfficialMcpEntry).command, ...(entry as OfficialMcpEntry).args].join(
                  " ",
                );
          return (
            <StoreCard
              key={entry.id}
              entry={entry as OfficialSkillEntry & Record<string, unknown>}
              meta={meta}
              status={status}
              action={action}
              busy={busyId === entry.id}
              busyText={s.installingShort}
              actionsDisabled={busyId !== null}
              scopeHint={scopeHint}
              onRun={async () => {
                if (!action) return;
                setBusyId(entry.id);
                setActionError(null);
                try {
                  await action.run();
                } catch (cause: unknown) {
                  setActionError(s.actionFailed(toMessage(cause, t.common.unexpected)));
                } finally {
                  setBusyId(null);
                }
              }}
            />
          );
        })}
      </ol>
    </div>
  );
}
