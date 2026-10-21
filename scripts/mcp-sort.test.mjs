/**
 * Tests for `sortMcps` in `src/lib/mcps.ts`.
 *
 * The MCP grid order is the operator's control surface for Enable All /
 * Disable All: `active` must float in-scope servers first, `official` must
 * float official servers first, and ties must never shuffle between renders.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { coversHarness, sortMcps } from "../src/lib/mcps.ts";

const server = (id, name, extra = {}) => ({
  id,
  name,
  description: `${name} server`,
  command: "npx",
  args: [],
  env: {},
  globalEnabled: false,
  enabledHarnesses: [],
  official: false,
  ...extra,
});

const names = (servers) => servers.map((server) => server.name);

test("active floats in-scope servers first, A-Z inside each band", () => {
  const servers = [
    server("b", "Bravo", { globalEnabled: true }),
    server("a", "Alpha"),
    server("c", "Charlie", { enabledHarnesses: ["codex"] }),
  ];
  assert.deepEqual(names(sortMcps(servers, "active", "global")), ["Bravo", "Alpha", "Charlie"]);
  assert.deepEqual(names(sortMcps(servers, "active", "codex")), ["Bravo", "Charlie", "Alpha"]);
});

test("alpha orders A-Z with an id tie-break", () => {
  const servers = [server("b2", "Bravo"), server("a", "Alpha"), server("b1", "Bravo")];
  assert.deepEqual(
    sortMcps(servers, "alpha", "global").map((server) => server.id),
    ["a", "b1", "b2"],
  );
});

test("alphaDesc orders Z-A", () => {
  const servers = [server("a", "Alpha"), server("c", "Charlie"), server("b", "Bravo")];
  assert.deepEqual(names(sortMcps(servers, "alphaDesc", "global")), ["Charlie", "Bravo", "Alpha"]);
});

test("official floats official servers first, A-Z inside each band", () => {
  const servers = [
    server("z", "Zulu", { official: true }),
    server("a", "Alpha", { official: true }),
    server("m", "Mike"),
  ];
  assert.deepEqual(names(sortMcps(servers, "official", "global")), ["Alpha", "Zulu", "Mike"]);
});

test("sort never mutates the input array", () => {
  const servers = [server("b", "Bravo"), server("a", "Alpha")];
  sortMcps(servers, "alpha", "global");
  assert.deepEqual(names(servers), ["Bravo", "Alpha"]);
});

test("coversHarness honors the global baseline plus harness opt-ins", () => {
  const global = server("g", "Global", { globalEnabled: true });
  const optIn = server("o", "OptIn", { enabledHarnesses: ["codex"] });
  const idle = server("i", "Idle");
  assert.equal(coversHarness(global, "global"), true);
  assert.equal(coversHarness(global, "codex"), true);
  assert.equal(coversHarness(optIn, "global"), false);
  assert.equal(coversHarness(optIn, "codex"), true);
  assert.equal(coversHarness(idle, "codex"), false);
});
