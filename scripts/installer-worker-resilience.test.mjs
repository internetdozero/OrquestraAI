import assert from "node:assert/strict";
import { test } from "node:test";

// Each import gets a fresh module, i.e. a cold isolate with an empty
// in-memory tag cache, which is exactly when GitHub outages used to 502.
let isolate = 0;
const coldWorker = async () =>
  (await import(`../workers/installer/worker.js?isolate=${(isolate += 1)}`)).default;

const LATEST = "https://github.com/internetdozero/ownhub/releases/latest";
const RAW = "https://raw.githubusercontent.com/internetdozero/ownhub/v1.2.3/install.sh";
const SCRIPT = "#!/bin/sh\necho ok\n";

function colo() {
  const store = new Map();
  const key = (k) => (typeof k === "string" ? k : k.url);
  return {
    store,
    default: {
      match: async (k) => store.get(key(k))?.clone() ?? null,
      put: async (k, res) => {
        store.set(key(k), res.clone());
      },
    },
  };
}

function github({ latest = [], raw = [] }) {
  const calls = [];
  const fetch = async (input) => {
    const url = String(input);
    calls.push(url);
    const queue = url === LATEST ? latest : url === RAW ? raw : null;
    if (!queue) throw new Error(`unexpected fetch: ${url}`);
    const next = queue.length > 1 ? queue.shift() : queue[0];
    if (next === "redirect") {
      return new Response(null, {
        status: 302,
        headers: { location: "https://github.com/internetdozero/ownhub/releases/tag/v1.2.3" },
      });
    }
    if (next === "script") return new Response(SCRIPT, { status: 200 });
    if (next === "network") throw new TypeError("network connection lost");
    return new Response("upstream error", { status: next });
  };
  return { calls, fetch };
}

async function run(worker, { method = "GET", url = "https://ownhub.example/install.sh" } = {}) {
  const pending = [];
  const res = await worker.fetch(
    new Request(url, { method }),
    {},
    {
      waitUntil: (p) => pending.push(p),
    },
  );
  await Promise.all(pending);
  return res;
}

async function withEnv(env, fn) {
  const saved = { fetch: globalThis.fetch, caches: globalThis.caches };
  globalThis.fetch = env.fetch;
  globalThis.caches = env.caches;
  try {
    return await fn();
  } finally {
    globalThis.fetch = saved.fetch;
    globalThis.caches = saved.caches;
  }
}

test("a transient GitHub 504 on the release lookup is retried instead of answering 502", async () => {
  const gh = github({ latest: [504, "redirect"], raw: ["script"] });
  const res = await withEnv({ fetch: gh.fetch, caches: colo() }, async () =>
    run(await coldWorker()),
  );
  assert.equal(res.status, 200);
  assert.equal(await res.text(), SCRIPT);
  assert.deepEqual(gh.calls, [LATEST, LATEST, RAW]);
});

test("transient raw.githubusercontent failures are retried too", async () => {
  const gh = github({ latest: ["redirect"], raw: ["network", 503, "script"] });
  const res = await withEnv({ fetch: gh.fetch, caches: colo() }, async () =>
    run(await coldWorker()),
  );
  assert.equal(res.status, 200);
  assert.equal(await res.text(), SCRIPT);
});

test("a cold isolate reuses the colo's shared tag and installer without calling GitHub", async () => {
  const caches = colo();
  const warm = github({ latest: ["redirect"], raw: ["script"] });
  await withEnv({ fetch: warm.fetch, caches }, async () => run(await coldWorker()));

  const down = github({ latest: [504], raw: [504] });
  const res = await withEnv({ fetch: down.fetch, caches }, async () => run(await coldWorker()));
  assert.equal(res.status, 200);
  assert.equal(await res.text(), SCRIPT);
  assert.equal(res.headers.get("x-ownhub-fallback"), null);
  assert.deepEqual(down.calls, []);
});

test("a GitHub outage serves the last good installer byte-identical instead of 502", async () => {
  const caches = colo();
  const warm = github({ latest: ["redirect"], raw: ["script"] });
  const first = await withEnv({ fetch: warm.fetch, caches }, async () => run(await coldWorker()));
  const etag = first.headers.get("etag");

  // The fresh entries expire; only the long-lived last-good copy remains.
  for (const k of [...caches.store.keys()]) if (!k.includes("/last-good/")) caches.store.delete(k);

  const down = github({ latest: [504], raw: [504] });
  for (const method of ["GET", "HEAD"]) {
    const res = await withEnv({ fetch: down.fetch, caches }, async () =>
      run(await coldWorker(), { method }),
    );
    assert.equal(res.status, 200, method);
    assert.equal(res.headers.get("etag"), etag);
    assert.equal(res.headers.get("x-ownhub-fallback"), "last-good");
    assert.equal(res.headers.get("x-ownhub-tag"), "v1.2.3");
    assert.equal(res.headers.get("content-type"), "text/x-shellscript; charset=utf-8");
    assert.equal(await res.text(), method === "GET" ? SCRIPT : "");
  }
});

test("with no copy anywhere, a persistent outage still reports 502 with the cause", async () => {
  const gh = github({ latest: [504], raw: ["script"] });
  const res = await withEnv({ fetch: gh.fetch, caches: colo() }, async () =>
    run(await coldWorker()),
  );
  assert.equal(res.status, 502);
  assert.equal(await res.text(), "installer unavailable: release tag lookup failed: HTTP 504");
  assert.deepEqual(gh.calls, [LATEST, LATEST, LATEST]);
});

test("HEAD on a warm colo answers from the cached copy instead of refetching", async () => {
  const caches = colo();
  const gh = github({ latest: ["redirect"], raw: ["script"] });
  await withEnv({ fetch: gh.fetch, caches }, async () => {
    const worker = await coldWorker();
    await run(worker);
    const head = await run(worker, { method: "HEAD" });
    assert.equal(head.status, 200);
    assert.equal(await head.text(), "");
  });
  assert.deepEqual(gh.calls, [LATEST, RAW]);
});
