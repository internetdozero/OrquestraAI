import type { ReactNode } from "react";
import { GRID_GAP, LIST_GAP } from "../lib/virtualRows";
import { cn } from "../lib/utils";
import type { SkillItem } from "../lib/skills";
import { useI18n } from "../hooks/useI18n";
import { VirtualCardGrid } from "./VirtualCardGrid";

/** Shared card grid: one column until there is room for two or three. */
export const SKILL_GRID_CLASS = "grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-3";

/** Shared list stack: compact rows, one per line. */
export const SKILL_LIST_CLASS = "flex flex-col gap-2";

/** Measured grid row: stabilized cards are a uniform ~190px. */
export const SKILL_GRID_ESTIMATE = 190;

/** Measured list row: compact rows are a uniform ~57px. */
export const SKILL_LIST_ESTIMATE = 57;

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
        className={cn("size-1.5 shrink-0 rounded-full", active ? "accent-dot" : "bg-zinc-600")}
      />
      <span
        className={cn(
          "text-[10.5px] font-medium tracking-[0.14em] uppercase",
          active ? "accent-text" : "text-zinc-500",
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
  /** Compact row used when `view` is `"list"`. */
  renderRow: (skill: SkillItem) => ReactNode;
  /** Grid and list render through the same mounted grids: switching views
   *  only swaps props, so the virtualization window (and scroll) survives. */
  view: "grid" | "list";
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
  renderRow,
  view,
  focusKey = null,
}: SkillGridProps & {
  activeSkills: SkillItem[];
  idleSkills: SkillItem[];
}) {
  const { t } = useI18n();
  const list = view === "list";
  const renderItem = list ? renderRow : renderCard;
  const className = list ? SKILL_LIST_CLASS : SKILL_GRID_CLASS;
  const estimate = list ? SKILL_LIST_ESTIMATE : SKILL_GRID_ESTIMATE;
  const gap = list ? LIST_GAP : GRID_GAP;

  return (
    <div className="space-y-5">
      {activeSkills.length > 0 && (
        <section className="accent-border accent-bg-soft rounded-2xl border p-3">
          <GridGroupLabel label={t.skills.activeGroup} count={activeSkills.length} active />
          <VirtualCardGrid
            items={activeSkills}
            keyOf={skillKey}
            renderItem={renderItem}
            className={className}
            estimate={estimate}
            gap={gap}
            focusKey={focusKey}
          />
        </section>
      )}

      {idleSkills.length > 0 && (
        <section
          className={cn(
            activeSkills.length > 0 && "rounded-2xl border border-white/[0.06] bg-zinc-900/20 p-3",
          )}
        >
          <GridGroupLabel label={t.skills.idleGroup} count={idleSkills.length} active={false} />
          <VirtualCardGrid
            items={idleSkills}
            keyOf={skillKey}
            renderItem={renderItem}
            className={className}
            estimate={estimate}
            gap={gap}
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
  renderRow,
  view,
  focusKey = null,
}: SkillGridProps & { skills: SkillItem[] }) {
  const list = view === "list";
  return (
    <VirtualCardGrid
      items={skills}
      keyOf={skillKey}
      renderItem={list ? renderRow : renderCard}
      className={list ? SKILL_LIST_CLASS : SKILL_GRID_CLASS}
      estimate={list ? SKILL_LIST_ESTIMATE : SKILL_GRID_ESTIMATE}
      gap={list ? LIST_GAP : GRID_GAP}
      focusKey={focusKey}
    />
  );
}
