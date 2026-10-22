import { invoke } from "@tauri-apps/api/core";
import { devMockMode } from "./fixtures.ts";

/** Masked key health from the Rust vault. */
export type VaultKeyHealth = "healthy" | "rateLimited" | "quotaExceeded" | "invalidKey";

/** One non-secret companion field (account id, region, project URL). */
export interface VaultMetaField {
  key: string;
  label: string;
  placeholder: string;
}

/** One preset in the blind-vault catalog. */
export interface VaultPreset {
  id: string;
  label: string;
  header: string;
  prefix: string;
  extraHeaders: [string, string][];
  authMirror?: string | null;
  envVar: string;
  description: string;
  /** Where the operator generates a key ("" = none). */
  keyUrl: string;
  /** Non-secret companion fields collected by the modal. */
  metaFields: VaultMetaField[];
}

/** Masked service: no plaintext secrets ever cross IPC. */
export interface VaultService {
  id: string;
  label: string;
  preset: string;
  description: string;
  activeKeyIndex: number;
  totalKeys: number;
  maskedKeys: string[];
  health: string;
  keyHealth: VaultKeyHealth[];
  /** Optional per-key tag (e.g. "workers", "d1"); parallels `maskedKeys`. */
  keyTags?: (string | null)[];
  /** Custom auth shaping (names only, never secrets). */
  customHeader?: string | null;
  customPrefix?: string | null;
  customEnv?: string | null;
  /** Cooldown expiry per key (epoch ms, 0 = none). */
  cooldownUntilMs: number[];
  /** Non-secret companion values, safe to display. */
  meta: Record<string, string>;
}

const MOCK_PRESET_TABLE: Array<[string, string, string, string]> = [
  ["openai", "OpenAI", "GPT models and embeddings.", "https://platform.openai.com/api-keys"],
  [
    "anthropic",
    "Anthropic",
    "Claude models via x-api-key.",
    "https://console.anthropic.com/settings/keys",
  ],
  [
    "gemini",
    "Google Gemini",
    "Gemini models via API key header.",
    "https://aistudio.google.com/apikey",
  ],
  ["groq", "Groq", "Ultra-low-latency inference.", "https://console.groq.com/keys"],
  [
    "deepseek",
    "DeepSeek",
    "DeepSeek chat and reasoning.",
    "https://platform.deepseek.com/api_keys",
  ],
  [
    "cloudflare",
    "Cloudflare",
    "Zone, DNS and Workers API.",
    "https://dash.cloudflare.com/profile/api-tokens",
  ],
  [
    "supabase",
    "Supabase",
    "Postgres, auth and storage.",
    "https://supabase.com/dashboard/account/tokens",
  ],
  ["aws", "AWS", "CLI/SDK use via env vars.", "https://console.aws.amazon.com/iam/home"],
  ["vercel", "Vercel", "Deployments and projects API.", "https://vercel.com/account/tokens"],
  [
    "neon",
    "Neon",
    "Serverless Postgres management.",
    "https://console.neon.tech/app/settings/api-keys",
  ],
  ["github", "GitHub", "Repos, issues and Actions.", "https://github.com/settings/tokens"],
  ["stripe", "Stripe", "Payments and billing API.", "https://dashboard.stripe.com/apikeys"],
  ["tavily", "Tavily", "Search API for agents.", "https://app.tavily.com"],
  ["deepgram", "Deepgram", "Speech-to-text and voice AI.", "https://console.deepgram.com"],
  ["jules", "Google Jules", "Async coding agent API.", "https://jules.google"],
  ["pix", "Pix", "Pix payment keys and tokens.", ""],
  ["unsplash", "Unsplash", "Stock photos API.", "https://unsplash.com/developers"],
  ["pixabay", "Pixabay", "Images and video API.", "https://pixabay.com/api/docs"],
  ["pexels", "Pexels", "Photos and video API.", "https://www.pexels.com/api"],
  ["exa", "Exa Web AI", "Neural search for agents.", "https://dashboard.exa.ai"],
  ["bluesky", "Bluesky", "AT Protocol app passwords.", "https://bsky.app/settings/app-passwords"],
  ["custom", "Custom / Generic", "Configurable header template or env var.", ""],
];

const mockPresets: VaultPreset[] = MOCK_PRESET_TABLE.map(([id, label, description, keyUrl]) => ({
  id,
  label,
  header: "authorization",
  prefix: "Bearer ",
  extraHeaders: id === "github" ? [["user-agent", "OrquestraAI"]] : [],
  envVar: `${id.toUpperCase()}_API_KEY`,
  description,
  keyUrl,
  metaFields: [],
}));

const mockServices: VaultService[] = [
  {
    id: "github-main",
    label: "GitHub Main",
    preset: "github",
    description: "CI and automation token pool.",
    activeKeyIndex: 0,
    totalKeys: 2,
    maskedKeys: ["ghp_abC...9f82", "ghp_xyZ...41de"],
    health: "Healthy",
    keyHealth: ["healthy", "healthy"],
    keyTags: [null, null],
    cooldownUntilMs: [0, 0],
    meta: {},
  },
  {
    id: "openai-pool",
    label: "OpenAI Pool",
    preset: "openai",
    description: "Inference keys with failover.",
    activeKeyIndex: 1,
    totalKeys: 2,
    maskedKeys: ["sk-pro...7a01", "sk-pro...c3b9"],
    health: "RateLimited",
    keyHealth: ["rateLimited", "healthy"],
    keyTags: [null, null],
    cooldownUntilMs: [Date.now() + 4 * 60_000, 0],
    meta: {},
  },
  {
    id: "cloudflare-multi",
    label: "Cloudflare Multi-API",
    preset: "cloudflare",
    description: "Scoped tokens per API surface.",
    activeKeyIndex: 0,
    totalKeys: 2,
    maskedKeys: ["cf_wor...1234", "cf_d1-...5678"],
    health: "Healthy",
    keyHealth: ["healthy", "healthy"],
    keyTags: ["workers", "d1"],
    cooldownUntilMs: [0, 0],
    meta: {},
  },
];

export function listVaultPresets(): Promise<VaultPreset[]> {
  if (devMockMode()) return Promise.resolve(mockPresets);
  return invoke<VaultPreset[]>("list_vault_presets");
}

export function vaultListServices(): Promise<VaultService[]> {
  if (devMockMode()) return Promise.resolve(mockServices);
  return invoke<VaultService[]>("vault_list_services");
}

export function vaultSaveService(input: {
  id: string;
  label: string;
  preset: string;
  description?: string;
  keys: string[];
  keyTags?: (string | null)[];
  customHeader?: string;
  customPrefix?: string;
  customEnv?: string;
  meta?: Record<string, string>;
}): Promise<VaultService> {
  return invoke<VaultService>("vault_save_service", {
    id: input.id,
    label: input.label,
    preset: input.preset,
    description: input.description ?? "",
    keys: input.keys,
    keyTags: input.keyTags ?? null,
    customHeader: input.customHeader ?? null,
    customPrefix: input.customPrefix ?? null,
    customEnv: input.customEnv ?? null,
    meta: input.meta ?? {},
  });
}

export function vaultDeleteService(id: string): Promise<void> {
  return invoke<void>("vault_delete_service", { id });
}

export function vaultSetPrimary(id: string, index: number): Promise<VaultService> {
  return invoke<VaultService>("vault_set_primary", { id, index });
}

// NOTE: the backend still registers `vault_http_request` and `vault_exec`,
// but no UI component may invoke blind key use: the two commands live in the
// `vault-blind-ops` permission set, which no capability grants to any window,
// so every invoke fails closed with a permission error. Re-exposing them
// means adding a capability file for `vault-blind-ops` AND a caller-side
// explicit user confirmation (what runs, with which service) mirroring the
// PurgeConfirmModal gate on `purge_sessions`.

/** Result of a live `vault_test_connection` ping. */
export interface VaultTestResult {
  success: boolean;
  latencyMs: number;
  message: string;
  health: string;
}

/**
 * Ping a pooled key against its preset's canonical verify endpoint.
 * Persists the observed health; the secret never crosses IPC.
 */
export function vaultTestConnection(service: string, keyIndex?: number): Promise<VaultTestResult> {
  return invoke<VaultTestResult>("vault_test_connection", {
    service,
    keyIndex: keyIndex ?? null,
  });
}

/** Remove one key from a pool (re-indexes, keeps a valid primary). */
export function vaultRemoveKey(id: string, index: number): Promise<VaultService> {
  return invoke<VaultService>("vault_remove_key", { id, index });
}

/** Per-file outcome of a migration-backup purge. */
export interface PurgeBackupsReport {
  vaultPurged: boolean;
  catalogPurged: boolean;
}

/**
 * Zeroize and unlink the `.migrate-bak` plaintext backups left by the
 * at-rest encryption migration. The backend only purges a backup whose
 * sealed file still opens, and reports per file; call it behind an
 * explicit user confirmation (see MigrationBackupPurgeModal).
 */
export function vaultPurgeMigrationBackups(): Promise<PurgeBackupsReport> {
  return invoke<PurgeBackupsReport>("vault_purge_migration_backups");
}
