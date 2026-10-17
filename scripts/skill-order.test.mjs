/**
 * Toggling a skill must not change its place in the default name order.
 * "Active first" still groups by state; that mode is opt-in.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { sortSkills } from "../src/lib/skills.ts";
import {
  skillsCacheGeneration,
  markSkillsCacheStale,
  dropSkillsCache,
  writeSkillsCache,
} from "../src/lib/skillsCache.ts";

const skill = (id, name, enabled) => ({
  id,
  name,
  description: "",
  category: "Core",
  tokens: 10,
  globalEnabled: enabled,
  harnessEnabled: enabled,
  inheritedFromGlobal: false,
});

test("name order stays put when a skill is switched off", () => {
  const items = [
    skill("rtk", "rtk", true),
    skill("alpha", "alpha", false),
    skill("zeta", "zeta", true),
  ];
  const before = sortSkills(items, "alpha").map((item) => item.id);
  const after = sortSkills(
    items.map((item) =>
      item.id === "rtk" ? { ...item, harnessEnabled: false, globalEnabled: false } : item,
    ),
    "alpha",
  ).map((item) => item.id);

  assert.deepEqual(before, ["alpha", "rtk", "zeta"]);
  assert.deepEqual(after, before);
});

test("a mutation bumps the cache generation so late reads can be told apart", () => {
  dropSkillsCache();
  const first = skillsCacheGeneration();
  writeSkillsCache("global", []);
  markSkillsCacheStale("global");
  assert.equal(skillsCacheGeneration(), first + 1);
  dropSkillsCache();
  assert.ok(skillsCacheGeneration() > first + 1);
});
