import assert from "node:assert/strict";
import { test } from "node:test";

import worker from "../workers/installer/worker.js";

test("installer worker resolves the latest release through the redirect", async () => {
  const originalFetch = globalThis.fetch;
  const originalCaches = globalThis.caches;
  const calls = [];
  globalThis.caches = {
    default: {
      match: async () => null,
      put: async () => {},
    },
  };
  globalThis.fetch = async (input, options) => {
    const url = String(input);
    calls.push({ url, options });
    if (url === "https://github.com/internetdozero/OrquestraAI/releases/latest") {
      return new Response(null, {
        status: 302,
        headers: {
          location: "https://github.com/internetdozero/OrquestraAI/releases/tag/v1.2.3",
        },
      });
    }
    if (url === "https://raw.githubusercontent.com/internetdozero/OrquestraAI/v1.2.3/install.sh") {
      return new Response("#!/bin/sh\necho ok\n", { status: 200 });
    }
    throw new Error(`unexpected fetch: ${url}`);
  };

  try {
    const pending = [];
    const response = await worker.fetch(
      new Request("https://installer.example.test/install.sh"),
      {},
      { waitUntil: (promise) => pending.push(promise) },
    );
    await Promise.all(pending);

    assert.equal(response.status, 200);
    assert.match(await response.text(), /echo ok/);
    assert.equal(calls.length, 2);
    assert.equal(calls[0].url, "https://github.com/internetdozero/OrquestraAI/releases/latest");
    assert.equal(calls[0].options.redirect, "manual");
    assert.equal(
      calls.some(({ url }) => url.includes("api.github.com")),
      false,
    );
  } finally {
    globalThis.fetch = originalFetch;
    globalThis.caches = originalCaches;
  }
});
