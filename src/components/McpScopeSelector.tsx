import { useI18n } from "../hooks/useI18n";
import { GLOBAL_SCOPE } from "../lib/harnesses";
import type { McpTarget } from "../lib/mcps";
import { ScopeTabs, type ScopeTab } from "./ScopeTabs";

/**
 * Scope selector for the MCP catalog: the toggle writes to whichever scope is
 * chosen here. Harnesses that are not installed stay listed but dimmed, so the
 * catalog reads the same on every machine.
 */
export function McpScopeSelector({
  scope,
  targets,
  onSelect,
}: {
  scope: string;
  targets: readonly McpTarget[];
  onSelect: (scope: string) => void;
}) {
  const { t } = useI18n();
  const tabs: ScopeTab[] = [
    {
      id: GLOBAL_SCOPE.id,
      label: GLOBAL_SCOPE.label,
      title: t.scopes.globalHint,
    },
    ...targets.map((target) => ({
      id: target.id,
      label: target.label,
      title: target.installed
        ? target.configPath
        : `${target.configPath} — ${t.common.notInstalled}`,
      dimmed: !target.installed,
      badge: !target.installed ? (
        <span className="font-mono text-[9px] text-zinc-700">
          {t.common.notInstalled}
        </span>
      ) : undefined,
    })),
  ];

  return (
    <ScopeTabs
      label={t.skills.scopeLabel}
      scope={scope}
      tabs={tabs}
      onSelect={onSelect}
    />
  );
}
