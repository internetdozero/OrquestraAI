import { invalidateCache, loadCacheValue, readCacheValue, reloadCacheValue } from "./dataCache";
import { listVaultPresets, vaultListServices, type VaultPreset, type VaultService } from "./vault";

export interface VaultCatalog {
  services: VaultService[];
  presets: VaultPreset[];
}

/**
 * Credentials are written by the core, the CLI and the MCP alike, so the
 * listing is worth revalidating; 5 min keeps the view responsive without
 * serving a rotated key's old metadata for a whole session.
 */
export const VAULT_CATALOG_KEY = "vault:catalog";
export const VAULT_CATALOG_TTL_MS = 5 * 60_000;

function loadCatalog(): Promise<VaultCatalog> {
  return Promise.all([vaultListServices(), listVaultPresets()]).then(([services, presets]) => ({
    services,
    presets,
  }));
}

/** Last vault listing this session, or `null` before the first read. */
export function readVaultCache(): VaultCatalog | null {
  return readCacheValue<VaultCatalog>(VAULT_CATALOG_KEY);
}

/** Drop the catalog so the next read talks to the core again. */
export function invalidateVaultCache(): void {
  invalidateCache(VAULT_CATALOG_KEY);
}

/** Resolve the shared vault listing, joining a read already on the way. */
export function ensureVaultCatalog(): Promise<VaultCatalog> {
  return loadCacheValue({
    key: VAULT_CATALOG_KEY,
    ttlMs: VAULT_CATALOG_TTL_MS,
    load: loadCatalog,
  });
}

/** Forget the cached listing and read it again. */
export function refreshVaultCatalog(): Promise<VaultCatalog> {
  return reloadCacheValue({
    key: VAULT_CATALOG_KEY,
    ttlMs: VAULT_CATALOG_TTL_MS,
    load: loadCatalog,
  });
}
