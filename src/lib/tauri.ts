import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { devMockMode, fixtureOfficialStore, fixtureSkills } from "./fixtures.ts";
import type { HarnessSummary } from "./harnesses";
import type { SkillItem } from "./skills";

export interface CoreStatus {
  name: string;
  version: string;
  platform: string;
  bridge: string;
}

function detectTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Ask the Rust core for its identity so the footer can prove the IPC bridge is
 * live. In a plain browser preview the invoke is skipped and `isTauri` is false.
 */
export function useCoreStatus() {
  const [status, setStatus] = useState<CoreStatus | null>(null);
  const [isTauri] = useState(detectTauriRuntime);

  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;

    invoke<CoreStatus>("core_status")
      .then((value) => {
        if (!cancelled) setStatus(value);
      })
      .catch(() => {
        if (!cancelled) setStatus(null);
      });

    return () => {
      cancelled = true;
    };
  }, [isTauri]);

  return { status, isTauri };
}

/** List skills evaluated for `harnessId`; `global` evaluates the baseline. */
export function listSkills(harnessId: string): Promise<SkillItem[]> {
  if (devMockMode()) {
    return Promise.resolve(fixtureSkills());
  }
  return invoke<SkillItem[]>("list_skills", {
    harnessId: harnessId === "global" ? null : harnessId,
  });
}

export function setSkillState(id: string, harnessId: string, enabled: boolean): Promise<void> {
  return invoke<void>("set_skill_state", { id, harnessId, enabled });
}

export function batchSetAll(harnessId: string, enabled: boolean): Promise<void> {
  return invoke<void>("batch_set_all", { harnessId, enabled });
}

/** Raw `SKILL.md` of one vault skill, for the in-app editor. */
export function readSkillContent(id: string): Promise<string> {
  return invoke<string>("read_skill_content", { id });
}

/** Atomically replace a skill's `SKILL.md`. */
export function saveSkillContent(id: string, content: string): Promise<void> {
  return invoke<void>("save_skill_content", { id, content });
}

/** Unlink a skill everywhere and remove its vault folder. */
export function deleteSkill(id: string): Promise<void> {
  return invoke<void>("delete_skill", { id });
}

export function harnessSummary(): Promise<HarnessSummary[]> {
  return invoke<HarnessSummary[]>("harness_summary");
}

/** Copy an external skill folder into the vault; rejects traversal and dupes. */
export function importSkill(sourcePath: string): Promise<SkillItem> {
  return invoke<SkillItem>("import_skill", { sourcePath });
}

/** Shallow-clone a git URL and import the skill it holds. */
export function importSkillFromGit(url: string): Promise<SkillItem> {
  return invoke<SkillItem>("import_skill_from_git", { url });
}

/** Tier badge for the Community & Official Hub. */
export type StoreTier = "official" | "community";

/** One curated skill in the Community Hub. `repoUrl` is absent for reserved
 * slots with no published repo yet. `official`/`tier`/`tags` drive the
 * filter chips; `hasScripts` drives the amber security badge. */
export interface OfficialSkillEntry {
  id: string;
  name: string;
  description: string;
  category: string;
  repoUrl?: string;
  official?: boolean;
  tier?: StoreTier;
  tags?: string[];
  hasScripts?: boolean;
  /** Pre-monorepo vault id satisfied by this entry in status matching. */
  legacyId?: string;
  /** Individual repo tried when the monorepo anchor is not yet published. */
  fallbackRepoUrl?: string;
}

/** One curated MCP server. `command`/`args` pre-fill a disabled catalog
 * entry the operator enables after installing the package. */
export interface OfficialMcpEntry {
  id: string;
  name: string;
  description: string;
  repoUrl?: string;
  command: string;
  args: string[];
  /** Pre-monorepo catalog id satisfied by this entry in status matching. */
  legacyId?: string;
  /** Individual repo tried when the monorepo anchor is not yet published. */
  fallbackRepoUrl?: string;
}

/** Curated Personal Store payload behind `list_official_store`. */
export interface OfficialStore {
  skills: OfficialSkillEntry[];
  mcps: OfficialMcpEntry[];
}

/** Fetch the curated Official Store (static data, never fails to exist). */
export function listOfficialStore(): Promise<OfficialStore> {
  if (devMockMode()) return Promise.resolve(fixtureOfficialStore());
  return invoke<OfficialStore>("list_official_store");
}

/** Result of pulling upstream into installed official skills & MCPs. */
export interface ExtensionSyncResult {
  updatedSkills: string[];
  updatedMcps: string[];
}

/** Download, verify and atomically install the desktop binary release. */
export function applyAppUpdate(targetVersion: string): Promise<string> {
  return invoke<string>("apply_app_update", { targetVersion });
}

/** Pull upstream into installed official skills; projections stay live. */
export function syncOfficialExtensions(): Promise<ExtensionSyncResult> {
  return invoke<ExtensionSyncResult>("sync_official_extensions");
}

/** Relaunch the app binary and exit this process. */
export function restartApp(): Promise<void> {
  return invoke<void>("restart_app");
}

/**
 * Report a React error-boundary crash to the Rust core (stderr, next to the
 * backend logs). No-op in browser previews, where there is no backend.
 */
export function logUiError(area: string, message: string, stack?: string): Promise<void> {
  if (!isTauri()) return Promise.resolve();
  return invoke<void>("log_ui_error", { area, message, stack: stack ?? null });
}

/** Native OS folder picker; `null` means the user cancelled. */
export function pickFolder(): Promise<string | null> {
  return invoke<string | null>("pick_folder");
}

/**
 * Push the window-close behavior to the Rust core: `true` hides to the
 * system tray on close, `false` exits the process. The shell calls this on
 * startup and on every `minimizeToTray` toggle.
 */
export function setMinimizeToTray(enabled: boolean): Promise<void> {
  return invoke<void>("set_minimize_to_tray", { enabled });
}

/** Reveal a `~/.orquestra` location in the native file manager. */
export function openInFileManager(path: string): Promise<void> {
  return invoke<void>("open_in_file_manager", { path });
}

/**
 * Snapshot `~/.orquestra` into `backups/orquestra_backup_<stamp>.tar.gz`.
 * Resolves to the archive path so the UI can toast the real destination.
 */
export function createOrquestraBackup(): Promise<string> {
  return invoke<string>("create_orquestra_backup");
}

/** True inside the Tauri webview; false in a plain browser preview. */
export function isTauri(): boolean {
  return detectTauriRuntime();
}

/**
 * Open an external link in the default system browser. The Tauri webview
 * blocks `<a target="_blank">`, so desktop goes through the `open_url`
 * command; browser previews fall back to `window.open`. A trailing `.git`
 * is stripped for clean web browsing (`repo.git` → `repo`).
 */
export async function openExternalUrl(url: string): Promise<void> {
  const webUrl = url.endsWith(".git") ? url.slice(0, -4) : url;
  if (isTauri()) {
    await invoke<void>("open_url", { url: webUrl });
  } else {
    window.open(webUrl, "_blank", "noreferrer,noopener");
  }
}

export function readMasterAgentsMd(): Promise<string> {
  return invoke<string>("read_master_agents_md");
}

export function writeMasterAgentsMd(content: string): Promise<void> {
  return invoke<void>("write_master_agents_md", { content });
}
