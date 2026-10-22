import { ensureAgentsText } from "./agentsCache";
import { HARNESSES, GLOBAL_SCOPE } from "./harnesses";
import { ensureMcpCatalog } from "./mcpCache";
import { ensureUnifiedSessions } from "./sessions";
import { hasSkillsCache } from "./skillsCache";
import { loadSkillsScope } from "./skillsLoader";

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
 * Warm the lazily-split view/modal chunks during idle time so the first
 * tab switch rarely hits the Suspense fallback. Fire-and-forget: dynamic
 * imports are cached by the module loader, and a rejection is ignored —
 * the fallback covers a cache miss.
 */
export async function preloadViewChunks(isCancelled: () => boolean): Promise<void> {
  await whenIdle(isCancelled);
  if (isCancelled()) return;
  await Promise.allSettled([
    import("../components/McpView"),
    import("../components/AgentsView"),
    import("../components/sessions/SessionsView"),
    import("../components/ideas/IdeasView"),
    import("../components/credentials/CredentialsView"),
    import("../components/JanitorModal"),
    import("../components/settings/SettingsModal"),
    import("../components/BridgeModal"),
  ]);
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

  await whenIdle(isCancelled);
  if (isCancelled()) return;
  await ensureAgentsText().catch(() => undefined);
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
