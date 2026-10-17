/**
 * Same heuristic as `estimate_tokens` in the Rust vault: about 3.8 characters
 * per token. Counts Unicode scalars so it stays next to `chars().count()`.
 */
const CHARS_PER_TOKEN = 3.8;

export function estimateTokens(content: string): number {
  if (content.length === 0) return 0;
  return Math.round(Array.from(content).length / CHARS_PER_TOKEN);
}

/** Name plus description: what a harness lists on every turn the item is on. */
export function listingTokens(name: string, description: string): number {
  return estimateTokens(`${name}\n${description}`.trim());
}

/**
 * Catalog definition we can weigh without talking to the server.
 * Env values stay out: they are process secrets, not prompt text.
 */
export function mcpCatalogTokens(server: {
  name: string;
  description: string;
  command: string;
  args: string[];
}): number {
  const full = estimateTokens(
    [server.name, server.description, server.command, server.args.join("\n")]
      .filter((part) => part.trim().length > 0)
      .join("\n"),
  );
  return Math.max(full, listingTokens(server.name, server.description));
}

/** Listing never reads heavier than the full body it was cut from. */
export function entryTokens(name: string, description: string, full: number): number {
  return Math.min(listingTokens(name, description), full);
}

/** Sum the listing and the catalog definition for the servers `include` keeps. */
export function sumMcpTokens<T extends {
  name: string;
  description: string;
  command: string;
  args: string[];
}>(servers: readonly T[], include: (server: T) => boolean): { entry: number; full: number } {
  let entry = 0;
  let full = 0;
  for (const server of servers) {
    if (!include(server)) continue;
    entry += listingTokens(server.name, server.description);
    full += mcpCatalogTokens(server);
  }
  return { entry, full };
}
