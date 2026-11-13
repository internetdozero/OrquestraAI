/**
 * Tests for the shared resource cache in `src/lib/dataCache.ts`.
 *
 * The rules that matter to the UI: a fresh entry is served without a read, an
 * expired one is read again, concurrent callers share one request, an
 * optimistic write survives a reply that started before it, and invalidation
 * makes the next read go back to the core.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  cacheAgeMs,
  invalidateCache,
  isCacheFresh,
  loadCacheValue,
  readCacheValue,
  reloadCacheValue,
  writeCacheValue,
} from "../src/lib/dataCache.ts";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** A loader that counts calls and answers with a fixed value. */
function counter(value) {
  const state = { calls: 0 };
  state.load = async () => {
    state.calls += 1;
    return value;
  };
  return state;
}

test("a fresh entry is served without calling the loader again", async () => {
  const key = "test:fresh";
  const loader = counter(["a"]);

  const first = await loadCacheValue({ key, ttlMs: 1_000, load: loader.load });
  const second = await loadCacheValue({ key, ttlMs: 1_000, load: loader.load });

  assert.deepEqual(first, ["a"]);
  assert.deepEqual(second, ["a"]);
  assert.equal(loader.calls, 1, "the TTL window must not re-read");
  assert.ok(isCacheFresh(key, 1_000));
  assert.deepEqual(readCacheValue(key), ["a"]);
});

test("an expired entry is read again and the age is reported", async () => {
  const key = "test:ttl";
  const loader = counter("first");
  await loadCacheValue({ key, ttlMs: 10, load: loader.load });

  await sleep(25);
  assert.ok(!isCacheFresh(key, 10), "the entry must be stale after its TTL");
  assert.ok(cacheAgeMs(key) >= 20);

  const loader2 = counter("second");
  const value = await loadCacheValue({ key, ttlMs: 10, load: loader2.load });
  assert.equal(value, "second");
  assert.equal(loader2.calls, 1);
});

test("concurrent callers join one in-flight read", async () => {
  const key = "test:single-flight";
  let calls = 0;
  const load = async () => {
    calls += 1;
    await sleep(10);
    return "shared";
  };

  const [a, b, c] = await Promise.all([
    loadCacheValue({ key, ttlMs: 1_000, load }),
    loadCacheValue({ key, ttlMs: 1_000, load }),
    loadCacheValue({ key, ttlMs: 1_000, load }),
  ]);

  assert.equal(a, "shared");
  assert.equal(b, "shared");
  assert.equal(c, "shared");
  assert.equal(calls, 1, "three mounts of the same view must not triplicate the read");
});

test("a forced reload bypasses the TTL but still joins an open request", async () => {
  const key = "test:force";
  const loader = counter("v1");
  await loadCacheValue({ key, ttlMs: 60_000, load: loader.load });

  const forced = counter("v2");
  assert.equal(await reloadCacheValue({ key, ttlMs: 60_000, load: forced.load }), "v2");
  assert.equal(forced.calls, 1);
  assert.equal(readCacheValue(key), "v2");
});

test("an optimistic write wins over a reply that started before it", async () => {
  const key = "test:optimistic";
  let release;
  const gate = new Promise((resolve) => {
    release = resolve;
  });
  const load = async () => {
    await gate;
    return [{ id: 1, isPinned: false }];
  };

  const pending = loadCacheValue({ key, ttlMs: 1_000, load });
  writeCacheValue(key, [{ id: 1, isPinned: true }]);
  release();
  await pending;

  assert.deepEqual(
    readCacheValue(key),
    [{ id: 1, isPinned: true }],
    "the stale reply must not overwrite the optimistic pin",
  );
});

test("invalidation drops the value so the next read reaches the core", async () => {
  const key = "test:invalidate";
  const loader = counter("cached");
  await loadCacheValue({ key, ttlMs: 60_000, load: loader.load });
  assert.equal(readCacheValue(key), "cached");

  invalidateCache(key);
  assert.equal(readCacheValue(key), null);

  const next = counter("fresh");
  assert.equal(await loadCacheValue({ key, ttlMs: 60_000, load: next.load }), "fresh");
  assert.equal(next.calls, 1);
});
