/**
 * Tests for `compareIdeas` in `src/lib/ideas.ts`.
 *
 * The list header sorts rows per section: text columns compare by locale,
 * priority follows low < medium < high < critical, and updated compares by
 * timestamp. Unknown priorities sink below `low` instead of breaking order.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { compareIdeas } from "../src/lib/ideas.ts";

const idea = (overrides = {}) => ({
  id: 1,
  project: "OrquestraAI",
  title: "Untitled",
  description: "",
  category: "feature",
  priority: "medium",
  status: "open",
  tags: "",
  createdAt: "2026-10-01 10:00:00",
  updatedAt: "2026-10-02 10:00:00",
  ...overrides,
});

const sign = (value) => (value === 0 ? 0 : value < 0 ? -1 : 1);

test("title, project and category compare by locale", () => {
  const a = idea({ title: "Alpha", project: "Zeta", category: "bug" });
  const b = idea({ title: "Bravo", project: "Alpha", category: "feature" });
  assert.equal(sign(compareIdeas("title", a, b)), -1);
  assert.equal(sign(compareIdeas("project", a, b)), 1);
  assert.equal(sign(compareIdeas("category", a, b)), -1);
  assert.equal(compareIdeas("title", a, a), 0);
});

test("priority follows low < medium < high < critical", () => {
  const order = ["low", "medium", "high", "critical"];
  for (let index = 0; index < order.length - 1; index += 1) {
    const lower = idea({ priority: order[index] });
    const higher = idea({ priority: order[index + 1] });
    assert.equal(sign(compareIdeas("priority", lower, higher)), -1);
    assert.equal(sign(compareIdeas("priority", higher, lower)), 1);
  }
  assert.equal(
    compareIdeas("priority", idea({ priority: "high" }), idea({ priority: "high" })),
    0,
  );
});

test("unknown priorities sink below low", () => {
  const weird = idea({ priority: "someday" });
  const low = idea({ priority: "low" });
  assert.equal(sign(compareIdeas("priority", weird, low)), -1);
  assert.equal(sign(compareIdeas("priority", low, weird)), 1);
});

test("updated compares by timestamp", () => {
  const older = idea({ updatedAt: "2026-10-01 10:00:00" });
  const newer = idea({ updatedAt: "2026-10-03 10:00:00" });
  assert.equal(sign(compareIdeas("updated", older, newer)), -1);
  assert.equal(sign(compareIdeas("updated", newer, older)), 1);
  assert.equal(compareIdeas("updated", older, { ...older }), 0);
});

test("sort with a negated comparator yields the exact reverse rank", () => {
  const rows = [
    idea({ id: 1, title: "Charlie" }),
    idea({ id: 2, title: "Alpha" }),
    idea({ id: 3, title: "Bravo" }),
  ];
  const asc = [...rows].sort((a, b) => compareIdeas("title", a, b));
  const desc = [...rows].sort((a, b) => -compareIdeas("title", a, b));
  assert.deepEqual(asc.map((row) => row.id), [2, 3, 1]);
  assert.deepEqual(desc.map((row) => row.id), [1, 3, 2]);
});
