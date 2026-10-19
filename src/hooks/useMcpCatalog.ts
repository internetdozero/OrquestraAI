import { useCallback, useEffect, useRef, useState } from "react";
import { devMockMode } from "../lib/fixtures";
import { useI18n } from "./useI18n";
import {
  ensureMcpCatalog,
  readMcpCache,
  refreshMcpCatalog,
  updateMcpServers,
} from "../lib/mcpCache";
import type { McpServer, McpTarget } from "../lib/mcps";

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

/**
 * MCP catalog for the registry. A warm cache paints immediately; the first
 * read and any later refresh share one in-flight request with the preload.
 */
export function useMcpCatalog(isTauri: boolean, reloadToken: number) {
  const { t } = useI18n();
  const cached = readMcpCache();
  // Browser mock fixtures resolve through the same cache calls, so every
  // guard below treats mock mode like a live core.
  const [mockMode] = useState(devMockMode);
  const active = isTauri || mockMode;
  const [servers, setServers] = useState<McpServer[]>(cached?.servers ?? []);
  const [targets, setTargets] = useState<McpTarget[]>(cached?.targets ?? []);
  const [loading, setLoading] = useState(active && cached === null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    async (force = false) => {
      if (!active) {
        setLoading(false);
        return;
      }

      if (!force && readMcpCache() === null) setLoading(true);
      try {
        const next = force ? await refreshMcpCatalog() : await ensureMcpCatalog();
        setServers(next.servers);
        setTargets(next.targets);
        setError(null);
      } catch (cause) {
        setError(t.mcps.readFailed(toMessage(cause, t.common.unexpected)));
      } finally {
        setLoading(false);
      }
    },
    [active, t],
  );

  useEffect(() => {
    void load(false);
  }, [load]);

  const lastReloadToken = useRef(reloadToken);
  useEffect(() => {
    if (reloadToken === lastReloadToken.current) return;

    lastReloadToken.current = reloadToken;
    void load(true);
  }, [reloadToken, load]);

  const replaceServers = (updater: (items: McpServer[]) => McpServer[]) => {
    setServers((previous) => {
      const next = updater(previous);
      updateMcpServers(next);
      return next;
    });
  };

  return { servers, targets, loading, error, setError, load, replaceServers };
}
