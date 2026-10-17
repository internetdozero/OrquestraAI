import { invoke } from "@tauri-apps/api/core";

/** How the bridge reaches a harness: projected skills or a config path list. */
export type HarnessBridgeKind = "skills" | "config" | "unsupported";

/** How the bridge neutralises a secondary discovery location. */
export type SecondaryAction = "project" | "quarantine" | "adoptOnly";

/** Mirrors the Rust `SecondaryStatus` struct. */
export interface SecondaryStatus {
  path: string;
  action: SecondaryAction;
  /** True when the location can no longer feed the harness unmanaged skills. */
  cleared: boolean;
  rationale: string;
}

/** Mirrors the Rust `HarnessStatus` struct. */
export interface HarnessStatus {
  id: string;
  label: string;
  kind: HarnessBridgeKind;
  /** Primary path the bridge owns for this harness. */
  target: string;
  skillsPaths: string[];
  skillsLinked: boolean;
  instructionsPath: string | null;
  instructionsLinked: boolean;
  configPath: string | null;
  configLinked: boolean;
  /** Secondary discovery locations this harness reads behind the projection. */
  secondaryPaths: SecondaryStatus[];
  /** True when every secondary location is claimed or archived. */
  secondaryCleared: boolean;
  installed: boolean;
  supported: boolean;
  bridged: boolean;
  note: string | null;
}

/** Mirrors the Rust `BridgeReport` struct. */
export interface BridgeReport {
  backupDir: string | null;
  bridged: number;
  total: number;
  harnesses: HarnessStatus[];
  /** Vault skills rescued from a harness before its folder was replaced. */
  adopted: string[];
  /** Secondary discovery locations emptied or replaced during this run. */
  quarantined: string[];
  instructionsImported: string[];
  warnings: string[];
  errors: string[];
}

export function getBridgeStatus(): Promise<HarnessStatus[]> {
  return invoke<HarnessStatus[]>("get_bridge_status");
}

export function deployHarnessBridge(): Promise<BridgeReport> {
  return invoke<BridgeReport>("deploy_harness_bridge");
}

/** `~`-shortened path for compact UI copy. */
export function shortenPath(path: string): string {
  const home = "/home/";
  if (path.startsWith(home)) {
    const rest = path.slice(home.length);
    const slash = rest.indexOf("/");
    return slash === -1 ? `~/${rest}` : `~/${rest.slice(slash + 1)}`;
  }
  return path;
}

export function backupName(path: string | null): string | null {
  if (!path) return null;
  return path.split("/").filter(Boolean).pop() ?? null;
}
