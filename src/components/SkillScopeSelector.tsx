import { GLOBAL_SCOPE, HARNESSES } from "../lib/harnesses";
import { cn } from "../lib/utils";
import { useI18n } from "../hooks/useI18n";
import { ScopeTabs, type ScopeTab } from "./ScopeTabs";

/** Active-skill pill; hidden until the vault totals are known. */
function CountPill({ value, active }: { value: number; active: boolean }) {
  return (
    <span
      className={cn(
        "rounded px-1 py-px font-mono text-[9px] tabular-nums",
        active ? "accent-bg-soft accent-text" : "bg-white/[0.05] text-zinc-500",
      )}
    >
      {value}
    </span>
  );
}

/**
 * Scope tabs for the skills grid: Global plus every harness the catalog knows,
 * each with the number of skills currently active there.
 *
 * Harnesses the bridge reports as missing stay listed but dimmed — the tab is
 * still selectable, because a projection can exist before the harness is
 * installed.
 */
export function SkillScopeSelector({
  scope,
  onSelect,
  counts,
  total,
  installedIds,
}: {
  scope: string;
  onSelect: (scope: string) => void;
  /** Active skills per scope id, from the core's harness summary. */
  counts: Record<string, number>;
  /** Catalog size; `0` hides the pills until the summary has loaded. */
  total: number;
  /** Installed harness ids, or `null` while the bridge status is unknown. */
  installedIds: ReadonlySet<string> | null;
}) {
  const { t } = useI18n();
  const showCounts = total > 0;

  const tabs: ScopeTab[] = [
    {
      id: GLOBAL_SCOPE.id,
      label: GLOBAL_SCOPE.label,
      icon: GLOBAL_SCOPE.icon,
      title: `${t.scopes.globalHint} · ${t.skills.activeSkillsCount(counts[GLOBAL_SCOPE.id] ?? 0, total)}`,
      badge: showCounts ? (
        <CountPill value={counts[GLOBAL_SCOPE.id] ?? 0} active={scope === GLOBAL_SCOPE.id} />
      ) : undefined,
    },
    ...HARNESSES.map((harness) => {
      const active = counts[harness.id] ?? 0;
      const dimmed = installedIds !== null && !installedIds.has(harness.id);

      return {
        id: harness.id,
        label: harness.label,
        icon: harness.icon,
        dimmed,
        title: `${harness.hint} · ${t.skills.activeSkillsCount(active, total)}${dimmed ? ` · ${t.skills.harnessNotInstalled}` : ""}`,
        badge: showCounts ? <CountPill value={active} active={scope === harness.id} /> : undefined,
      };
    }),
  ];

  return <ScopeTabs label={t.skills.scopeLabel} scope={scope} tabs={tabs} onSelect={onSelect} />;
}
