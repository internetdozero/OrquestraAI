import type { ReactNode } from "react";
import { cn } from "../lib/utils";
import type { SkillItem } from "../lib/skills";
import { useI18n } from "../hooks/useI18n";
import { VirtualCardGrid } from "./VirtualCardGrid";

/** Shared card grid: one column until there is room for two or three. */
export const SKILL_GRID_CLASS =
  "grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-3";

const skillKey = (skill: SkillItem) => skill.id;

/** Section heading that separates the active block from the idle tail. */
function GridGroupLabel({
  label,
  count,
  active,
}: {
  label: string;
  count: number;
  active: boolean;
}) {
  return (
    <div className="mb-2 flex items-center gap-2">
      <span
        className={cn(
          "size-1.5 shrink-0 rounded-full",
          active ? "bg-emerald-400" : "bg-zinc-600",
        )}
      />
      <span
        className={cn(
          "text-[10.5px] font-medium tracking-[0.14em] uppercase",
          active ? "text-emerald-300" : "text-zinc-500",
        )}
      >
        {label}
      </span>
      <span className="font-mono text-[10.5px] text-zinc-600">{count}</span>
      <span className="ml-1 h-px flex-1 bg-white/[0.06]" />
    </div>
  );
}

interface SkillGridProps {
  renderCard: (skill: SkillItem) => ReactNode;
  /** Card to reveal and flash after it crossed between the groups. */
  focusKey?: string | null;
}

/**
 * Grouped grid: enabled skills above, idle ones below, each windowed on its
 * own. The idle block loses its frame when it is the only group, so an
 * all-idle vault does not render a pointless nested panel.
 */
export function GroupedSkillGrid({
  activeSkills,
  idleSkills,
  renderCard,
  focusKey = null,
}: SkillGridProps & {
  activeSkills: SkillItem[];
  idleSkills: SkillItem[];
}) {
  const { t } = useI18n();

  return (
    <div className="space-y-5">
      {activeSkills.length > 0 && (
        <section className="rounded-2xl border border-emerald-500/[0.12] bg-emerald-500/[0.02] p-3">
          <GridGroupLabel label={t.skills.activeGroup} count={activeSkills.length} active />
          <VirtualCardGrid
            items={activeSkills}
            keyOf={skillKey}
            renderItem={renderCard}
            className={SKILL_GRID_CLASS}
            focusKey={focusKey}
          />
        </section>
      )}

      {idleSkills.length > 0 && (
        <section
          className={cn(
            activeSkills.length > 0 &&
              "rounded-2xl border border-white/[0.06] bg-zinc-900/20 p-3",
          )}
        >
          <GridGroupLabel label={t.skills.idleGroup} count={idleSkills.length} active={false} />
          <VirtualCardGrid
            items={idleSkills}
            keyOf={skillKey}
            renderItem={renderCard}
            className={SKILL_GRID_CLASS}
            focusKey={focusKey}
          />
        </section>
      )}
    </div>
  );
}

/** Flat grid used by every non-grouped sort mode. */
export function FlatSkillGrid({
  skills,
  renderCard,
  focusKey = null,
}: SkillGridProps & { skills: SkillItem[] }) {
  return (
    <VirtualCardGrid
      items={skills}
      keyOf={skillKey}
      renderItem={renderCard}
      className={SKILL_GRID_CLASS}
      focusKey={focusKey}
    />
  );
}
