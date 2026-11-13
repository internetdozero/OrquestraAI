/**
 * Tests for the opt-in timings in `src/lib/perf.ts`.
 *
 * The contract the view instrumentation relies on: nothing is recorded unless
 * enabled, one view switch produces exactly one entry tagged with where its
 * first data came from, and a paint for a view that was never switched to is
 * ignored instead of inventing a duration.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  perfBoot,
  perfClear,
  perfEnabled,
  perfReport,
  perfViewPainted,
  perfViewSwitch,
  setPerfEnabled,
} from "../src/lib/perf.ts";

test("nothing is recorded while disabled", () => {
  setPerfEnabled(false);
  perfClear();

  perfViewSwitch("ideas");
  perfViewPainted("ideas", "cache");
  perfBoot("shell");

  assert.deepEqual(perfReport(), []);
  assert.equal(perfEnabled(), false);
});

test("a switch records one entry tagged with its data source", () => {
  setPerfEnabled(true);
  perfClear();

  perfViewSwitch("sessions");
  perfViewPainted("sessions", "cache");

  const entries = perfReport();
  assert.equal(entries.length, 1);
  assert.equal(entries[0].name, "view:sessions");
  assert.equal(entries[0].source, "cache");
  assert.ok(entries[0].ms >= 0);
  assert.ok(entries[0].at.length > 0);
});

test("later paints of the same switch are ignored", () => {
  setPerfEnabled(true);
  perfClear();

  perfViewSwitch("ideas");
  perfViewPainted("ideas", "cache");
  perfViewPainted("ideas", "load");

  assert.equal(perfReport().length, 1, "a background revalidation must not count again");
});

test("a paint without a preceding switch is ignored", () => {
  setPerfEnabled(true);
  perfClear();

  perfViewPainted("mcps", "cache");

  assert.equal(perfReport().length, 0);
});

test("a paint for a different view than the switch is ignored", () => {
  setPerfEnabled(true);
  perfClear();

  perfViewSwitch("credentials");
  perfViewPainted("agents", "load");

  assert.equal(perfReport().length, 0);
});

test("boot phases are measured from module load and keep their order", () => {
  setPerfEnabled(true);
  perfClear();

  perfBoot("shell");
  perfBoot("preload-done");

  const [shell, preload] = perfReport();
  assert.equal(shell.name, "boot:shell");
  assert.equal(preload.name, "boot:preload-done");
  assert.ok(preload.ms >= shell.ms);
});

test("disabling stops collection again", () => {
  setPerfEnabled(true);
  perfClear();
  perfBoot("shell");
  assert.equal(perfReport().length, 1);

  setPerfEnabled(false);
  perfBoot("preload-done");
  assert.equal(perfReport().length, 1);
});
