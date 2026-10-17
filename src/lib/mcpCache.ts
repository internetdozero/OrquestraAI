import { listMcpServers, listMcpTargets, type McpServer, type McpTarget } from "./mcps";

export interface McpCatalog {
  servers: McpServer[];
  targets: McpTarget[];
}

let catalog: McpCatalog | null = null;
let inflight: Promise<McpCatalog> | null = null;

/** Last catalog the shell fetched or edited, or `null` before the first read. */
export function readMcpCache(): McpCatalog | null {
  return catalog;
}

/** Keep a local edit visible the next time the registry mounts. */
export function updateMcpServers(servers: McpServer[]): void {
  catalog = { servers, targets: catalog?.targets ?? [] };
}

/** Drop the catalog so the next read talks to the core again. */
export function invalidateMcpCache(): void {
  catalog = null;
  inflight = null;
}

/** Resolve the shared catalog, joining a read that is already on the way. */
export function ensureMcpCatalog(): Promise<McpCatalog> {
  if (catalog) return Promise.resolve(catalog);
  if (inflight) return inflight;

  inflight = Promise.all([listMcpServers(), listMcpTargets()])
    .then(([servers, targets]) => {
      // A toggle during the read already stored the optimistic servers.
      catalog = catalog ? { servers: catalog.servers, targets } : { servers, targets };
      return catalog;
    })
    .finally(() => {
      inflight = null;
    });

  return inflight;
}

/** Forget the cached catalog and read it again. */
export function refreshMcpCatalog(): Promise<McpCatalog> {
  invalidateMcpCache();
  return ensureMcpCatalog();
}
