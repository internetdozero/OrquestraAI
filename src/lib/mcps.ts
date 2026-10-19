import { invoke } from "@tauri-apps/api/core";

/** Mirrors the Rust `McpServer` struct. */
export interface McpServer {
  id: string;
  name: string;
  description: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  globalEnabled: boolean;
  enabledHarnesses: string[];
  /** Computed official tier from the Rust core; absent in older payloads. */
  official?: boolean;
}

/** Mirrors the Rust `McpTargetStatus` struct. */
export interface McpTarget {
  id: string;
  label: string;
  configPath: string;
  installed: boolean;
}

export type ExecutionKind = "npx" | "python" | "binary";

/**
 * Classify how a server is launched, which is the only thing that decides
 * whether a machine can actually run it. We look at the basename so
 * `/usr/bin/npx` reads the same as `npx`.
 */
export function executionKind(command: string): ExecutionKind {
  const base = command.trim().split("/").pop()?.toLowerCase() ?? "";
  if (base === "npx" || base === "bunx" || base === "pnpx") return "npx";
  if (base === "python" || base === "python3" || base === "uvx") return "python";
  return "binary";
}

/** How the MCP grid orders cards within the selected scope. */
export type McpSortMode = "active" | "alpha" | "alphaDesc" | "official";

/** Case-insensitive name order, with an id tie-break so ties never shuffle. */
function compareMcpNames(left: McpServer, right: McpServer): number {
  const byName = left.name.localeCompare(right.name, undefined, {
    sensitivity: "base",
  });
  return byName !== 0 ? byName : left.id.localeCompare(right.id);
}

/**
 * Order the grid. `active` floats every in-scope server above the rest (A–Z
 * inside each band); `official` floats official servers first (A–Z inside).
 */
export function sortMcps(
  servers: McpServer[],
  mode: McpSortMode,
  scope: string,
): McpServer[] {
  const ranked = [...servers];
  switch (mode) {
    case "alpha":
      return ranked.sort(compareMcpNames);
    case "alphaDesc":
      return ranked.sort((left, right) => compareMcpNames(right, left));
    case "official":
      return ranked.sort((left, right) => {
        const leftOfficial = left.official === true ? 0 : 1;
        const rightOfficial = right.official === true ? 0 : 1;
        return leftOfficial - rightOfficial || compareMcpNames(left, right);
      });
    case "active":
    default:
      return ranked.sort((left, right) => {
        const leftActive = coversHarness(left, scope) ? 0 : 1;
        const rightActive = coversHarness(right, scope) ? 0 : 1;
        return leftActive - rightActive || compareMcpNames(left, right);
      });
  }
}

/** `global` or a harness that inherits the baseline on top of its own opt-ins. */
export function coversHarness(server: McpServer, harnessId: string): boolean {
  if (harnessId === "global") return server.globalEnabled;
  return (
    server.globalEnabled || server.enabledHarnesses.includes(harnessId)
  );
}

/** True when the scope inherits the server and so cannot switch it off here. */
export function isLockedByGlobal(
  server: McpServer,
  harnessId: string,
): boolean {
  return harnessId !== "global" && server.globalEnabled;
}

/** Environment keys whose values are masked in the drawer by default. */
export function isSecretKey(key: string): boolean {
  return /(KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|COOKIE|AUTH|BEARER)/i.test(key);
}

export function isSecretValue(value: string): boolean {
  return /^(?:sk-|ghp_|cf_|\*{4}|•{4})/i.test(value);
}

export function isEnvironmentTemplate(value: string): boolean {
  return /\$(?:\{[A-Za-z_][A-Za-z0-9_]*(?::-.*?)?\}|[A-Za-z_][A-Za-z0-9_]*)/.test(
    value,
  );
}

/** Full command line as the harness will execute it. */
export function commandLine(server: McpServer): string {
  return [server.command, ...server.args].join(" ");
}

/** `KEY=VALUE` lines, one per environment variable. */
export function parseEnv(text: string): Record<string, string> {
  const env: Record<string, string> = {};

  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;

    const split = line.indexOf("=");
    if (split <= 0) continue;
    env[line.slice(0, split).trim()] = line.slice(split + 1).trim();
  }

  return env;
}

/** One `KEY=VALUE` per line, sorted so the editor round-trips predictably. */
export function formatEnv(env: Record<string, string>): string {
  return Object.keys(env)
    .sort()
    .map((key) => `${key}=${env[key]}`)
    .join("\n");
}

/** `mcp_canva` / `Canva Docs` → a stable slug the harnesses accept as a key. */
export function slugify(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function listMcpServers(): Promise<McpServer[]> {
  return invoke<McpServer[]>("list_mcps");
}

export function listMcpTargets(): Promise<McpTarget[]> {
  return invoke<McpTarget[]>("mcp_targets");
}

/** Enable or disable every server for one scope in a single atomic write. */
export function batchSetMcpAll(
  harnessId: string | null,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("batch_set_mcp_all", {
    harnessId: harnessId === "global" ? null : harnessId,
    enabled,
  });
}

/** `harnessId` `null` (or `global`) edits the universal baseline. */
export function setMcpState(
  id: string,
  harnessId: string | null,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("set_mcp_state", {
    id,
    harnessId: harnessId === "global" ? null : harnessId,
    enabled,
  });
}

export function saveMcp(server: McpServer): Promise<void> {
  return invoke<void>("save_mcp", { mcpServer: server });
}

export function deleteMcp(id: string): Promise<void> {
  return invoke<void>("delete_mcp", { id });
}

/** Fetch a single value only after the operator explicitly requests reveal. */
export function revealMcpSecret(id: string, key: string): Promise<string> {
  return invoke<string>("reveal_mcp_secret", { id, key });
}

/** Re-compile every harness config; resolves with the changed file paths. */
export function deployMcps(): Promise<string[]> {
  return invoke<string[]>("deploy_mcps");
}
