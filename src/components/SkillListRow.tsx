import { memo } from "react";
import { Loader2, Lock, Pencil, Trash2 } from "lucide-react";
import { skillIcon, type SkillItem } from "../lib/skills";
import { entryTokens } from "../lib/tokens";
import { formatTokens } from "../lib/utils";
import { useI18n } from "../hooks/useI18n";
import { OfficialBadge } from "./OfficialBadge";

interface SkillListRowProps {
  skill: SkillItem;
  onToggle: (id: string) => void;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
  busy?: boolean;
  deleteBusy?: boolean;
}

/**
 * Single-line summary for the skills list view; the toggle, edit and
 * delete actions match the card. Full detail stays in the card grid.
 */
export const SkillListRow = memo(function SkillListRow({
  skill,
  onToggle,
  onEdit,
  onDelete,
  busy = false,
  deleteBusy = false,
}: SkillListRowProps) {
  const { t } = useI18n();
  const Icon = skillIcon(skill.category);
  const active = skill.harnessEnabled;
  const inherited = skill.inheritedFromGlobal;

  return (
    <div
      data-card-id={skill.id}
      aria-busy={busy}
      className="flex items-center gap-2.5 rounded-xl border border-white/[0.07] bg-zinc-900/60 px-3 py-2.5"
    >
      <span className="grid size-8 shrink-0 place-items-center rounded-lg border border-white/[0.07] bg-white/[0.03] text-zinc-500">
        <Icon className="size-4" />
      </span>
      <div className="w-44 min-w-0 shrink-0">
        <h3 className="truncate font-mono text-[13px] font-medium text-zinc-100" title={skill.name}>
          {skill.name}
        </h3>
        <p className="truncate text-[10px] font-medium tracking-wide text-zinc-500 uppercase">
          {skill.category}
        </p>
      </div>
      {skill.official === true && <OfficialBadge />}
      <p className="hidden min-w-0 flex-1 truncate text-[12px] text-zinc-400 lg:block">
        {skill.description}
      </p>
      <span
        className="hidden shrink-0 font-mono text-[10.5px] text-zinc-500 md:inline"
        title={t.tokens.fullSkillHint}
      >
        ≈ {formatTokens(entryTokens(skill.name, skill.description, skill.tokens))}
      </span>
      {inherited ? (
        <span className="accent-text flex shrink-0 items-center gap-1 text-[10px] font-medium">
          <Lock className="size-2.5" />
          {t.skills.inheritedFromGlobal}
        </span>
      ) : (
        <span
          className={`flex shrink-0 items-center gap-1 text-[10px] font-medium ${active ? "accent-text" : "text-zinc-600"}`}
        >
          <span className={`size-1 rounded-full ${active ? "accent-dot" : "bg-zinc-600"}`} />
          {active ? t.skills.activePill : t.skills.idlePill}
        </span>
      )}
      <div className="flex shrink-0 items-center gap-1">
        <button
          type="button"
          onClick={() => onEdit(skill.id)}
          disabled={busy}
          title={t.skills.editSkill}
          aria-label={`${t.skills.editSkill}: ${skill.name}`}
          className="rounded-md p-1.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200 disabled:opacity-40"
        >
          <Pencil className="size-3.5" />
        </button>
        <button
          type="button"
          onClick={() => onDelete(skill.id)}
          disabled={busy || deleteBusy}
          title={t.skills.deleteSkill}
          aria-label={`${t.skills.deleteSkill}: ${skill.name}`}
          className="rounded-md p-1.5 text-zinc-500 transition hover:bg-red-500/10 hover:text-red-300 disabled:opacity-40"
        >
          {deleteBusy ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : (
            <Trash2 className="size-3.5" />
          )}
        </button>
        <button
          type="button"
          role="switch"
          aria-checked={active}
          aria-label={`${active ? t.skills.disableVerb : t.skills.enableVerb} ${skill.name}`}
          title={inherited ? t.skills.inheritedLockTitle : undefined}
          disabled={busy || inherited}
          onClick={() => onToggle(skill.id)}
          className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors duration-200 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 focus-visible:outline-none ${
            active
              ? "border-[var(--accent-ring)] bg-[var(--accent-solid)]"
              : "border-white/10 bg-zinc-800 hover:bg-zinc-700"
          } ${busy ? "cursor-wait opacity-60" : ""} ${inherited ? "cursor-not-allowed" : ""}`}
        >
          <span
            className={`block size-3.5 rounded-full bg-white shadow-sm transition-transform duration-200 ${
              active ? "translate-x-[18px]" : "translate-x-[3px]"
            }`}
          />
        </button>
      </div>
    </div>
  );
});
