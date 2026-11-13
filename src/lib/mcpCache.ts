import {
  invalidateCache,
  loadCacheValue,
  readCacheValue,
  reloadCacheValue,
  writeCacheValue,
} from "./dataCache";
import { listMcpServers, listMcpTargets, type McpServer, type McpTarget } from "./mcps";

export interface McpCatalog {
  servers: McpServer[];
  targets: McpTarget[];
}

/** Registry reads are cheap but not free; 60 s covers a tab switch. */
export const MCP_CATALOG_KEY = "mcp:catalog";
export const MCP_CATALOG_TTL_MS = 60_000;

/**
 * One catalog read: servers plus their per-harness targets.
 *
 * Another agent can edit `mcp.json` while the app is open, so the entry
 * expires on a TTL instead of living for the whole session; every mutation in
 * the UI invalidates it explicitly, which is the case that must be immediate.
 */
function loadCatalog(): Promise<McpCatalog> {
  return Promise.all([listMcpServers(), listMcpTargets()]).then(([servers, targets]) => ({
    servers,
    targets,
  }));
}

/** Last catalog the shell fetched or edited, or `null` before the first read. */
export function readMcpCache(): McpCatalog | null {
  return readCacheValue<McpCatalog>(MCP_CATALOG_KEY);
}

/**
 * Keep a local edit visible the next time the registry mounts.
 *
 * Writing through the cache bumps its generation, so a read that started
 * before the toggle can no longer overwrite the optimistic list.
 */
export function updateMcpServers(servers: McpServer[]): void {
  const cached = readMcpCache();
  writeCacheValue<McpCatalog>(MCP_CATALOG_KEY, {
    servers,
    targets: cached?.targets ?? [],
  });
}

/** Drop the catalog so the next read talks to the core again. */
export function invalidateMcpCache(): void {
  invalidateCache(MCP_CATALOG_KEY);
}

/** Resolve the shared catalog, joining a read that is already on the way. */
export function ensureMcpCatalog(): Promise<McpCatalog> {
  return loadCacheValue({
    key: MCP_CATALOG_KEY,
    ttlMs: MCP_CATALOG_TTL_MS,
    load: loadCatalog,
  });
}

/** Forget the cached catalog and read it again. */
export function refreshMcpCatalog(): Promise<McpCatalog> {
  return reloadCacheValue({
    key: MCP_CATALOG_KEY,
    ttlMs: MCP_CATALOG_TTL_MS,
    load: loadCatalog,
  });
}
