import { ensureAgentsText } from "./agentsCache";
import { HARNESSES, GLOBAL_SCOPE } from "./harnesses";
import { loadBlackboardNotes, loadIdeasCatalog } from "./ideas";
import { ensureMcpCatalog } from "./mcpCache";
import { ensureVaultCatalog } from "./vaultCache";
import { ensureUnifiedSessions, loadRetentionPolicy } from "./sessions";
import { hasSkillsCache } from "./skillsCache";
import { loadSkillsScope } from "./skillsLoader";
import { ensureRtkStatus } from "./rtkCache";
import { ensureStoreCatalog } from "./storeCache";
import { probeExternalTools } from "./tooling";

/** What the first screen needs before the shell is shown. */
export function preloadEssentials(): Promise<void> {
  return loadSkillsScope(GLOBAL_SCOPE.id).then(() => undefined);
}

/**
 * One idle window before doing work: the browser fires when the main thread
 * is quiet, `timeout` forces the slot even under sustained interaction, and
 * the `setTimeout` fallback covers runtimes without the API. Resolves
 * immediately when the effect was cancelled so callers can bail uniformly.
 */
function whenIdle(isCancelled: () => boolean, timeoutMs = 3000): Promise<void> {
  return new Promise((resolve) => {
    if (isCancelled()) {
      resolve();
      return;
    }
    const idle = window as Window & {
      requestIdleCallback?: (callback: () => void, options?: { timeout: number }) => number;
    };
    if (typeof idle.requestIdleCallback === "function") {
      idle.requestIdleCallback(() => resolve(), { timeout: timeoutMs });
    } else {
      window.setTimeout(() => resolve(), 0);
    }
  });
}

/**
 * Everything the settings dialog reads: the retention policy, the tool
 * probe, the RTK version and the store catalog. All four are cached with a
 * TTL, so callers can warm them repeatedly for the cost of one read.
 */
function settingsReads(): Promise<unknown>[] {
  return [loadRetentionPolicy(), probeExternalTools(), ensureRtkStatus(), ensureStoreCatalog()];
}

/**
 * Warm the settings dialog ahead of a click: its chunk plus every read its
 * tabs need. Fired on hover/focus of the sidebar button.
 */
export function preloadSettings(): Promise<void> {
  return Promise.allSettled([
    import("../components/settings/SettingsModal"),
    ...settingsReads(),
  ]).then(() => undefined);
}

/**
 * Warm the lazily-split view/modal chunks during idle time so the first
 * tab switch rarely hits the Suspense fallback. Fire-and-forget: dynamic
 * imports are cached by the module loader, and a rejection is ignored —
 * the fallback covers a cache miss.
 *
 * The settings dialog is the exception: it hangs off the sidebar footer, so
 * it can be clicked at any moment — including before the first idle window,
 * which is why clicking it used to dim the whole window behind the Suspense
 * fallback. Start its chunk immediately; a module fetch is async and never
 * blocks the thread.
 */
export async function preloadViewChunks(isCancelled: () => boolean): Promise<void> {
  void import("../components/settings/SettingsModal").catch(() => undefined);
  await whenIdle(isCancelled);
  if (isCancelled()) return;
  await Promise.allSettled([
    import("../components/McpView"),
    import("../components/AgentsView"),
    import("../components/sessions/SessionsView"),
    import("../components/ideas/IdeasView"),
    import("../components/credentials/CredentialsView"),
    import("../components/JanitorModal"),
    import("../components/BridgeModal"),
  ]);
}

/**
 * Warm the data behind one view, for a hover or keyboard focus on its sidebar
 * entry: a click that follows lands on a filled cache instead of a skeleton.
 * Never rejects — a failed warm just means the view loads itself as before.
 */
export async function preloadViewData(view: string): Promise<void> {
  switch (view) {
    case "ideas":
      await Promise.all([loadIdeasCatalog(), loadBlackboardNotes()]);
      return;
    case "sessions":
      await Promise.all([ensureUnifiedSessions(), loadRetentionPolicy()]);
      return;
    case "mcps":
      await ensureMcpCatalog();
      return;
    case "credentials":
      await ensureVaultCatalog();
      return;
    case "agents":
      await ensureAgentsText();
      return;
    case "skills":
      // The grid's first screen is already warm; re-evaluating the vault on
      // every hover would be pure waste.
      if (!hasSkillsCache(GLOBAL_SCOPE.id)) await loadSkillsScope(GLOBAL_SCOPE.id);
      return;
    default:
      return;
  }
}

/**
 * Everything else, one idle window at a time, after the shell is on screen.
 * Boot used to fire MCP + agents + the full session scan + every harness
 * scope back-to-back the moment the shell painted, saturating I/O while the
 * user was trying to interact. Now each step waits for the main thread to
 * go quiet, so warming never contends with input, scrolling or animation.
 *
 * Nothing here is load-bearing: every view lazy-loads its own data on mount
 * with a skeleton (see useSkillsCache, useMcpCatalog, useSessions), and the
 * caches dedupe concurrent reads, so a tab switch mid-warm simply joins the
 * in-flight request instead of doubling it. Harness scopes the operator
 * already visited are skipped outright.
 */
export async function preloadBackground(isCancelled: () => boolean): Promise<void> {
  // Nearby views first: the registries behind one click, each a single read.
  await whenIdle(isCancelled);
  if (isCancelled()) return;
  await ensureMcpCatalog().catch(() => undefined);
  if (isCancelled()) return;

  // Settings and the official store are one click away and each of their
  // tabs opened on a cold read; warming them here keeps every tab from
  // flashing its defaults while the IPC resolves.
  await whenIdle(isCancelled);
  if (isCancelled()) return;
  await Promise.allSettled(settingsReads());
  if (isCancelled()) return;

  await whenIdle(isCancelled);
  if (isCancelled()) return;
  // Ideas is one click away and used to be the only view with no warm-up at
  // all: every visit paid three round trips behind a skeleton. Filling its
  // cache entry here makes the first open paint from memory.
  await Promise.all([loadIdeasCatalog(), loadBlackboardNotes()]).catch(() => undefined);
  if (isCancelled()) return;

  await whenIdle(isCancelled);
  if (isCancelled()) return;
  await ensureAgentsText().catch(() => undefined);
  if (isCancelled()) return;

  await whenIdle(isCancelled);
  if (isCancelled()) return;
  await ensureVaultCatalog().catch(() => undefined);
  if (isCancelled()) return;

  // Index the session catalog while the user is still on the first screen,
  // so opening Sessions paints from memory (see `peekWarmedSessions`).
  // This is the heavy step (~1.2k session files); it only runs when idle.
  await whenIdle(isCancelled);
  if (isCancelled()) return;
  await ensureUnifiedSessions().catch(() => undefined);

  // Harness scopes last, one idle window each, skipping whatever the grid
  // already loaded (a first tab switch warms its own scope on demand).
  for (const harness of HARNESSES) {
    if (isCancelled()) return;
    if (hasSkillsCache(harness.id)) continue;
    await whenIdle(isCancelled);
    if (isCancelled()) return;
    await loadSkillsScope(harness.id).catch(() => undefined);
  }
}
