import { ensureAgentsText } from "./agentsCache";
import { HARNESSES, GLOBAL_SCOPE } from "./harnesses";
import { ensureMcpCatalog } from "./mcpCache";
import { ensureUnifiedSessions } from "./sessions";
import { loadSkillsScope } from "./skillsLoader";

/** What the first screen needs before the shell is shown. */
export function preloadEssentials(): Promise<void> {
  return loadSkillsScope(GLOBAL_SCOPE.id).then(() => undefined);
}

/**
 * Everything else, one step at a time, after the shell is on screen.
 * MCP and the master instructions go first because those views are one click
 * away; harness scopes follow so a tab switch paints from memory.
 */
export async function preloadBackground(
  isCancelled: () => boolean,
): Promise<void> {
  if (isCancelled()) return;

  await ensureMcpCatalog().catch(() => undefined);
  if (isCancelled()) return;

  await ensureAgentsText().catch(() => undefined);
  if (isCancelled()) return;

  // Index the session catalog while the user is still on the first screen,
  // so opening Sessions paints from memory (see `peekWarmedSessions`).
  await ensureUnifiedSessions().catch(() => undefined);

  for (const harness of HARNESSES) {
    if (isCancelled()) return;
    await loadSkillsScope(harness.id).catch(() => undefined);
  }
}
