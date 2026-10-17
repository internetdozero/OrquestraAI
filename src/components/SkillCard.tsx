import { Globe, Link2, Lock, Unlink2 } from "lucide-react";
import { skillIcon, type SkillItem } from "../lib/skills";
import { entryTokens } from "../lib/tokens";
import { cn } from "../lib/utils";
import { useI18n } from "../hooks/useI18n";
import { TokenPair } from "./TokenPair";

interface SkillCardProps {
  skill: SkillItem;
  onToggle: (id: string) => void;
  busy?: boolean;
  /** Briefly highlights a card that just crossed between the grid sections. */
  flash?: boolean;
}

function Toggle({
  checked,
  onChange,
  label,
  disabled,
  locked,
}: {
  checked: boolean;
  onChange: () => void;
  label: string;
  disabled: boolean;
  locked: boolean;
}) {
  const { t } = useI18n();

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={
        locked
          ? `${label} — ${t.skills.inheritedFromGlobal}`
          : `${checked ? t.skills.disableVerb : t.skills.enableVerb} ${label}`
      }
      title={locked ? t.skills.inheritedLockTitle : undefined}
      disabled={disabled || locked}
      onClick={onChange}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors duration-200 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 focus-visible:outline-none",
        checked
          ? "border-emerald-400/50 bg-emerald-500/90"
          : "border-white/10 bg-zinc-800 hover:bg-zinc-700",
        disabled && "cursor-wait opacity-60",
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

export function SkillCard({
  skill,
  onToggle,
  busy = false,
  flash = false,
}: SkillCardProps) {
  const { t } = useI18n();
  const Icon = skillIcon(skill.category);
  const active = skill.harnessEnabled;
  const inherited = skill.inheritedFromGlobal;

  return (
    <article
      data-card-id={skill.id}
      aria-busy={busy}
      className={cn(
        // Only paint properties transition: `transition-all` also animated
        // layout/geometry and made WebKitGTK re-lay-out every card under the
        // cursor while the grid scrolled.
        "group relative flex flex-col gap-3 overflow-hidden rounded-xl border p-4",
        "transition-[border-color,background-color,box-shadow] duration-150 ease-out",
        // Hover emphasis is visual only. The old `-translate-y-px` moved the
        // card out from under the pointer, which cancelled the hover and
        // immediately re-applied it — a 60fps flicker loop.
        "hover:shadow-[0_8px_30px_-12px_rgba(0,0,0,0.9)]",
        // Rasterize each card once and keep its repaints contained so a hover
        // or a flash cannot cascade into its grid siblings.
        "transform-gpu [backface-visibility:hidden] [contain:content]",
        active
          ? "border-emerald-500/20 bg-gradient-to-b from-emerald-500/[0.045] to-zinc-900/40 hover:border-emerald-500/35"
          : "border-white/[0.07] bg-zinc-900/30 hover:border-white/15 hover:bg-zinc-900/50",
        flash && "border-emerald-400/60 shadow-[0_0_0_3px_rgba(52,211,153,0.16)]",
        busy && "opacity-80",
      )}
    >
      {active && (
        <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-emerald-400/60 to-transparent" />
      )}

      <div className="flex items-start gap-3">
        <div
          className={cn(
            "grid size-9 shrink-0 place-items-center rounded-lg border transition-colors",
            active
              ? "border-emerald-500/25 bg-emerald-500/10 text-emerald-300"
              : "border-white/[0.07] bg-white/[0.03] text-zinc-500",
          )}
        >
          <Icon className="size-4" />
        </div>

        <div className="min-w-0 flex-1">
          <h3
            className="truncate font-mono text-[13px] font-medium text-zinc-100"
            title={skill.name}
          >
            {skill.name}
          </h3>
          <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
            <span className="rounded border border-white/[0.07] bg-white/[0.03] px-1.5 py-px text-[10px] font-medium tracking-wide text-zinc-400 uppercase">
              {skill.category}
            </span>
            {inherited ? (
              <span className="flex items-center gap-1 text-[9.5px] font-medium text-emerald-300/90">
                <Lock className="size-2.5" />
                {t.skills.inheritedFromGlobal}
              </span>
            ) : active ? (
              <span className="flex items-center gap-1 text-[10px] font-medium text-emerald-400">
                <span className="size-1 rounded-full bg-emerald-400" />
                {t.skills.activePill}
              </span>
            ) : (
              <span className="flex items-center gap-1 text-[10px] text-zinc-600">
                <span className="size-1 rounded-full bg-zinc-600" />
                {t.skills.idlePill}
              </span>
            )}
          </div>
        </div>

        <Toggle
          checked={active}
          onChange={() => onToggle(skill.id)}
          label={skill.name}
          disabled={busy}
          locked={inherited}
        />
      </div>

      <p className="line-clamp-2 text-[12.5px] leading-relaxed text-zinc-400">
        {skill.description}
      </p>

      <div className="mt-auto flex flex-wrap items-center justify-between gap-x-3 gap-y-1 border-t border-white/[0.05] pt-3 font-mono text-[10.5px] text-zinc-500">
        <TokenPair
          entry={entryTokens(skill.name, skill.description, skill.tokens)}
          full={skill.tokens}
          fullHint={t.tokens.fullSkillHint}
        />
        <span
          className={cn(
            "flex items-center gap-1.5",
            active ? "text-emerald-400/80" : "text-zinc-600",
          )}
        >
          {inherited ? (
            <Globe className="size-3" />
          ) : active ? (
            <Link2 className="size-3" />
          ) : (
            <Unlink2 className="size-3" />
          )}
          {inherited
            ? t.skills.globalProjection
            : active
              ? t.skills.symlinkedHere
              : t.skills.notProjected}
        </span>
      </div>
    </article>
  );
}
