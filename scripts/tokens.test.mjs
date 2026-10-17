/**
 * Prompt-weight estimates. The divisor matches `estimate_tokens` in the Rust
 * vault (380 characters → 100). Listing weight is name + description; a skill
 * listing never exceeds the full file the core already measured.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  entryTokens,
  estimateTokens,
  listingTokens,
  mcpCatalogTokens,
  sumMcpTokens,
} from "../src/lib/tokens.ts";

test("matches the vault heuristic, counting Unicode scalars", () => {
  assert.equal(estimateTokens(""), 0);
  assert.equal(estimateTokens("x".repeat(380)), 100);
  // One emoji is one scalar and two UTF-16 units. UTF-16 length would round to 1.
  assert.equal(estimateTokens("👍"), 0);
});

test("skill listing is clamped to the full file", () => {
  const name = "Alpha";
  const description = "Short listing for alpha.";
  assert.equal(listingTokens(name, description), 8);
  assert.equal(entryTokens(name, description, 500), 8);
  assert.equal(entryTokens(name, description, 3), 3);
});

test("MCP catalog weight ignores env secrets and stays at least the listing", () => {
  const base = {
    name: "github",
    description: "Repo tools",
    command: "npx",
    args: ["-y", "@scope/server"],
  };
  const withSecret = { ...base, env: { TOKEN: "x".repeat(5000) } };

  assert.equal(listingTokens(base.name, base.description), 4);
  assert.equal(mcpCatalogTokens(base), 10);
  assert.equal(mcpCatalogTokens(withSecret), 10);

  const nameOnly = { name: "bare", description: "", command: "", args: [] };
  assert.equal(mcpCatalogTokens(nameOnly), listingTokens("bare", ""));
});

test("scope sum keeps only the servers the predicate includes", () => {
  const on = {
    name: "github",
    description: "Repo tools",
    command: "npx",
    args: ["-y", "@scope/server"],
    globalEnabled: true,
  };
  const off = {
    name: "idle",
    description: "Not projected",
    command: "python",
    args: ["-m", "unused"],
    globalEnabled: false,
  };

  const sum = sumMcpTokens([on, off], (server) => server.globalEnabled);
  assert.deepEqual(sum, { entry: 4, full: 10 });
});
