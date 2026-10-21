import { useCallback } from "react";
import { useI18n } from "./useI18n";
import { coversHarness, type McpServer } from "../lib/mcps";
import type { OfficialMcpEntry, OfficialSkillEntry } from "../lib/tauri";
import type { StoreEntry, StoreRowAction, StoreStatus } from "../components/OfficialStore";

interface McpStoreDeps {
  scope: string;
  toggle: (server: McpServer) => Promise<void>;
  save: (server: McpServer) => Promise<void>;
  reload: () => Promise<void>;
  onToast: (message: string) => void;
}

/** Live status + one-click actions backing the MCP store tab. */
export function useMcpStoreActions(servers: McpServer[], deps: McpStoreDeps) {
  const { t } = useI18n();
  const { scope, toggle, save, reload, onToast } = deps;

  const statusOf = useCallback(
    (entry: StoreEntry): StoreStatus => {
      // Legacy catalog ids satisfy the unified entry: no duplicate Add
      // button for servers saved under their pre-monorepo id.
      const legacy = "legacyId" in entry ? entry.legacyId : undefined;
      const found = servers.find(
        (server) => server.id === entry.id || (legacy && server.id === legacy),
      );
      if (!found) return "missing";
      return coversHarness(found, scope) ? "active" : "idle";
    },
    [servers, scope],
  );

  const actionFor = useCallback(
    (entry: OfficialSkillEntry | OfficialMcpEntry, status: StoreStatus): StoreRowAction | null => {
      if (status === "idle") {
        const legacy = "legacyId" in entry ? entry.legacyId : undefined;
        const found = servers.find(
          (server) => server.id === entry.id || (legacy && server.id === legacy),
        );
        if (!found) return null;
        return { kind: "enable", run: () => toggle(found) };
      }
      // A missing official server is added disabled: the operator installs
      // the package, then enables it. Inactive servers never reach the
      // harness configs, so a not-yet-installed binary cannot break them.
      if (status === "missing" && "command" in entry) {
        const template = entry;
        return {
          kind: "add",
          run: async () => {
            await save({
              id: template.id,
              name: template.name,
              description: template.description,
              command: template.command,
              args: [...template.args],
              env: {},
              globalEnabled: false,
              enabledHarnesses: [],
            });
            onToast(t.mcps.savedDeployed(template.name));
            await reload();
          },
        };
      }
      return null;
    },
    [t, servers, toggle, save, reload, onToast],
  );

  return { statusOf, actionFor };
}
