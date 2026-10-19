import {
  AlertTriangle,
  Download,
  ExternalLink,
  Info,
  Loader2,
  Plus,
  Power,
  ShieldCheck,
} from "lucide-react";
import { openExternalUrl, type OfficialSkillEntry } from "../lib/tauri";
import { useI18n } from "../hooks/useI18n";
import { cn } from "../lib/utils";
import type { StoreRowAction, StoreStatus } from "./OfficialStore";

const ACTION_ICON = {
  install: Download,
  enable: Power,
  add: Plus,
} as const;

interface StoreCardProps {
  entry: OfficialSkillEntry & Record<string, unknown>;
  meta: string;
  status: StoreStatus;
  action: StoreRowAction | null;
  busy: boolean;
  onRun: () => Promise<void>;
  scopeHint?: string;
}

function tierOf(entry: OfficialSkillEntry): "official" | "community" {
  if (entry.tier === "community" || entry.tier === "official") return entry.tier;
  return entry.official === false ? "community" : "official";
}

/** One hub row: security badge, description, tags, repo link and action. */
export function StoreCard({
  entry,
  meta,
  status,
  action,
  busy,
  onRun,
  scopeHint,
}: StoreCardProps) {
  const { t } = useI18n();
  const s = t.store;
  const ActionIcon = action ? ACTION_ICON[action.kind] : null;
  const tier = tierOf(entry);
  const hasScripts = entry.hasScripts === true;
  const tags = Array.isArray(entry.tags) ? (entry.tags as string[]) : [];

  return (
    <li className="flex items-start gap-3 px-4 py-3 transition-[border-color,background-color,box-shadow] [contain:content] hover:bg-white/[0.02]">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
          <span className="truncate font-mono text-[12.5px] font-medium text-zinc-100">
            {entry.name}
          </span>
          {tier === "official" ? (
            <span className="flex items-center gap-1 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-1.5 py-px text-[10px] font-medium text-emerald-300">
              <ShieldCheck className="size-3" />
              {s.verifiedOfficial}
            </span>
          ) : hasScripts ? (
            <span
              className="flex items-center gap-1 rounded-full border border-amber-500/30 bg-amber-500/10 px-1.5 py-px text-[10px] font-medium text-amber-300"
              title={s.scriptsWarning(tags.length > 0 ? tags.length : 1)}
            >
              <AlertTriangle className="size-3" />
              {s.communityWithScripts}
            </span>
          ) : (
            <span className="flex items-center gap-1 rounded-full border border-white/10 bg-white/[0.04] px-1.5 py-px text-[10px] font-medium text-zinc-400">
              <Info className="size-3" />
              {s.communityPromptOnly}
            </span>
          )}
          {status === "missing" ? (
            <span className="text-[10px] text-zinc-600">{t.common.notInstalled}</span>
          ) : (
            <span className="text-[10px] font-medium text-emerald-400">
              {status === "active" ? s.projected : s.installed}
            </span>
          )}
        </div>
        <p className="mt-0.5 line-clamp-2 text-[12px] leading-relaxed text-zinc-400">
          {entry.description}
        </p>
        <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[10px] text-zinc-600">
          <span className="rounded border border-white/[0.07] bg-white/[0.03] px-1.5 py-px">
            {meta}
          </span>
          {tags.slice(0, 3).map((tag) => (
            <span key={tag} className="text-zinc-600">
              #{tag}
            </span>
          ))}
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
        {action?.kind === "install" && scopeHint && entry.repoUrl && (
          <p className="mt-1 text-[10.5px] text-zinc-600">{scopeHint}</p>
        )}
      </div>

      {action && ActionIcon && (
        <button
          type="button"
          disabled={busy}
          onClick={() => void onRun()}
          title={action.kind === "install" ? scopeHint : undefined}
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
          {action.kind === "install"
            ? s.installAndProject
            : action.kind === "enable"
              ? s.enable
              : s.add}
        </button>
      )}
    </li>
  );
}
