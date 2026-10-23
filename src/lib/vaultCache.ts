import { listVaultPresets, vaultListServices, type VaultPreset, type VaultService } from "./vault";

export interface VaultCatalog {
  services: VaultService[];
  presets: VaultPreset[];
}

let catalog: VaultCatalog | null = null;
let inflight: Promise<VaultCatalog> | null = null;

/** Last vault listing this session, or `null` before the first read. */
export function readVaultCache(): VaultCatalog | null {
  return catalog;
}

/** Drop the catalog so the next read talks to the core again. */
export function invalidateVaultCache(): void {
  catalog = null;
  inflight = null;
}

/** Resolve the shared vault listing, joining a read already on the way. */
export function ensureVaultCatalog(): Promise<VaultCatalog> {
  if (catalog) return Promise.resolve(catalog);
  if (inflight) return inflight;

  inflight = Promise.all([vaultListServices(), listVaultPresets()])
    .then(([services, presets]) => {
      catalog = { services, presets };
      return catalog;
    })
    .finally(() => {
      inflight = null;
    });

  return inflight;
}

/** Forget the cached listing and read it again. */
export function refreshVaultCatalog(): Promise<VaultCatalog> {
  invalidateVaultCache();
  return ensureVaultCatalog();
}
