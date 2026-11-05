import { useI18n } from "../hooks/useI18n";
import { GLOBAL_SCOPE } from "../lib/harnesses";
import { coversHarness, type McpServer, type McpTarget } from "../lib/mcps";
import { ScopeCountBadge, ScopeTabs, type ScopeTab } from "./ScopeTabs";

/**
 * Scope selector for the MCP catalog: the toggle writes to whichever scope is
 * chosen here. Harnesses that are not installed stay listed but dimmed, so the
 * catalog reads the same on every machine.
 */
export function McpScopeSelector({
  scope,
  servers,
  targets,
  showCounts,
  onSelect,
}: {
  scope: string;
  servers: readonly McpServer[];
  targets: readonly McpTarget[];
  showCounts: boolean;
  onSelect: (scope: string) => void;
}) {
  const { t } = useI18n();
  const countFor = (scopeId: string) =>
    servers.filter((server) => coversHarness(server, scopeId)).length;
  const globalCount = countFor(GLOBAL_SCOPE.id);
  const tabs: ScopeTab[] = [
    {
      id: GLOBAL_SCOPE.id,
      label: GLOBAL_SCOPE.label,
      title: `${t.scopes.globalHint} · ${t.mcps.activeMcpCount(globalCount)}`,
      badge: showCounts ? (
        <ScopeCountBadge value={globalCount} active={scope === GLOBAL_SCOPE.id} />
      ) : undefined,
    },
    ...targets.map((target) => ({
      id: target.id,
      label: target.label,
      title: `${
        target.installed ? target.configPath : `${target.configPath} — ${t.common.notInstalled}`
      } · ${t.mcps.activeMcpCount(countFor(target.id))}`,
      dimmed: !target.installed,
      badge: showCounts ? (
        <ScopeCountBadge value={countFor(target.id)} active={scope === target.id} />
      ) : !target.installed ? (
        <span className="font-mono text-[9px] text-zinc-700">{t.common.notInstalled}</span>
      ) : undefined,
    })),
  ];

  return <ScopeTabs label={t.skills.scopeLabel} scope={scope} tabs={tabs} onSelect={onSelect} />;
}
