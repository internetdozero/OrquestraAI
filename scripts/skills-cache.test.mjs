/**
 * Tests for `src/lib/skillsCache.ts` and the optimistic projection helper.
 *
 * The cache is the thing that makes a scope switch instant, so its staleness
 * rules are load-bearing: a Global toggle must mark every harness stale, and
 * dropping the cache must never leave a stale entry behind.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  dropSkillsCache,
  hasSkillsCache,
  isSkillsCacheStale,
  markSkillsCacheStale,
  projectSkillState,
  readSkillsCache,
  writeSkillsCache,
} from "../src/lib/skillsCache.ts";

const GLOBAL = "global";

const skill = (id, overrides = {}) => ({
  id,
  name: id,
  description: `${id} description`,
  category: "Security",
  tokens: 120,
  globalEnabled: false,
  harnessEnabled: false,
  inheritedFromGlobal: false,
  ...overrides,
});

const reset = () => dropSkillsCache();

test("a written scope is served from cache and starts fresh", () => {
  reset();
  const items = [skill("a"), skill("b")];
  writeSkillsCache("global", items);

  assert.equal(hasSkillsCache("global"), true);
  assert.equal(readSkillsCache("global"), items);
  assert.equal(isSkillsCacheStale("global"), false);
  assert.equal(readSkillsCache("codex"), null);
  assert.equal(hasSkillsCache("codex"), false);
});

test("marking stale keeps every scope servable", () => {
  reset();
  writeSkillsCache("global", [skill("a")]);
  writeSkillsCache("codex", [skill("a")]);
  writeSkillsCache("claude", [skill("a")]);

  markSkillsCacheStale("global");

  assert.equal(isSkillsCacheStale("global"), false, "the toggled scope stays fresh");
  assert.equal(isSkillsCacheStale("codex"), true);
  assert.equal(isSkillsCacheStale("claude"), true);
  assert.equal(readSkillsCache("codex")?.length, 1, "stale entries are still served");
});

test("marking every scope stale uses no exception", () => {
  reset();
  writeSkillsCache("global", [skill("a")]);
  writeSkillsCache("codex", [skill("a")]);

  markSkillsCacheStale();

  assert.equal(isSkillsCacheStale("global"), true);
  assert.equal(isSkillsCacheStale("codex"), true);
});

test("writing a scope again clears its stale flag", () => {
  reset();
  writeSkillsCache("codex", [skill("a")]);
  markSkillsCacheStale();
  assert.equal(isSkillsCacheStale("codex"), true);

  writeSkillsCache("codex", [skill("a"), skill("b")]);

  assert.equal(isSkillsCacheStale("codex"), false);
  assert.equal(readSkillsCache("codex")?.length, 2);
});

test("dropping one scope leaves the others intact", () => {
  reset();
  writeSkillsCache("global", [skill("a")]);
  writeSkillsCache("codex", [skill("a")]);

  dropSkillsCache("codex");

  assert.equal(hasSkillsCache("codex"), false);
  assert.equal(hasSkillsCache("global"), true);
});

test("dropping everything empties the cache", () => {
  reset();
  writeSkillsCache("global", [skill("a")]);
  writeSkillsCache("codex", [skill("a")]);

  dropSkillsCache();

  assert.equal(hasSkillsCache("global"), false);
  assert.equal(hasSkillsCache("codex"), false);
  assert.equal(readSkillsCache("global"), null);
});

test("a Global toggle projects into the global and harness flags", () => {
  const enabled = projectSkillState(skill("a"), GLOBAL, true, GLOBAL);
  assert.equal(enabled.globalEnabled, true);
  assert.equal(enabled.harnessEnabled, true);
  assert.equal(enabled.inheritedFromGlobal, false);

  const inherited = skill("b", {
    globalEnabled: true,
    inheritedFromGlobal: true,
  });
  const released = projectSkillState(inherited, GLOBAL, false, GLOBAL);
  assert.equal(released.globalEnabled, false);
  assert.equal(released.harnessEnabled, false);
  assert.equal(released.inheritedFromGlobal, false, "a Global change re-locks nothing");
});

test("a harness toggle only moves that harness's flag", () => {
  const inherited = skill("b", {
    globalEnabled: true,
    harnessEnabled: true,
    inheritedFromGlobal: true,
  });

  const updated = projectSkillState(inherited, "codex", false, GLOBAL);

  assert.equal(updated.harnessEnabled, false);
  assert.equal(updated.globalEnabled, true, "Global is untouched");
  assert.equal(updated.inheritedFromGlobal, true, "inheritance bookkeeping is preserved");
});
