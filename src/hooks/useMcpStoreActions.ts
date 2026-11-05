import { useCallback } from "react";
import { useI18n } from "./useI18n";
import { coversHarness, type McpServer } from "../lib/mcps";
import type { OfficialMcpEntry, OfficialSkillEntry } from "../lib/tauri";
import type { StoreEntry, StoreRowAction, StoreStatus } from "../components/OfficialStore";

interface McpStoreDeps {
  scope: string;
  toggle: (server: McpServer) => Promise<void>;
  save: (server: McpServer) => Promise<boolean>;
  getSaveError: () => string | null;
  onToast: (message: string) => void;
}

/** Live status + one-click actions backing the MCP store tab. */
export function useMcpStoreActions(servers: McpServer[], deps: McpStoreDeps) {
  const { t } = useI18n();
  const { scope, toggle, save, getSaveError, onToast } = deps;

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
      // Saving it enabled compiles the server into the selected scope.
      if (status === "missing" && "command" in entry) {
        const template = entry;
        return {
          kind: "add",
          run: async () => {
            onToast(t.store.installing(template.name));
            const global = scope === "global";
            const saved = await save({
              id: template.id,
              name: template.name,
              description: template.description,
              command: template.command,
              args: [...template.args],
              env: {},
              globalEnabled: global,
              enabledHarnesses: global ? [] : [scope],
            });
            if (!saved) throw new Error(getSaveError() ?? t.store.mcpSaveFailed);
          },
        };
      }
      return null;
    },
    [t, servers, toggle, save, getSaveError, onToast, scope],
  );

  return { statusOf, actionFor };
}
