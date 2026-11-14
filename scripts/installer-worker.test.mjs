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
    if (url === "https://github.com/OwnAnAI/ownhub/releases/latest") {
      return new Response(null, {
        status: 302,
        headers: {
          location: "https://github.com/OwnAnAI/ownhub/releases/tag/v1.2.3",
        },
      });
    }
    if (url === "https://raw.githubusercontent.com/OwnAnAI/ownhub/v1.2.3/install.sh") {
      return new Response("#!/bin/sh\necho ok\n", { status: 200 });
    }
    if (url === "https://raw.githubusercontent.com/OwnAnAI/ownhub/v1.2.3/install.ps1") {
      return new Response("Write-Host ok\n", { status: 200 });
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
    assert.equal(calls[0].url, "https://github.com/OwnAnAI/ownhub/releases/latest");
    assert.equal(calls[0].options.redirect, "manual");
  } finally {
    globalThis.fetch = originalFetch;
    globalThis.caches = originalCaches;
  }
});

test("installer worker mirrors the PowerShell installer from the same tag", async () => {
  const originalFetch = globalThis.fetch;
  const originalCaches = globalThis.caches;
  const calls = [];
  globalThis.caches = {
    default: {
      match: async () => null,
      put: async () => {},
    },
  };
  globalThis.fetch = async (input) => {
    const url = String(input);
    calls.push(url);
    if (url === "https://github.com/OwnAnAI/ownhub/releases/latest") {
      return new Response(null, {
        status: 302,
        headers: {
          location: "https://github.com/OwnAnAI/ownhub/releases/tag/v1.2.3",
        },
      });
    }
    if (url === "https://raw.githubusercontent.com/OwnAnAI/ownhub/v1.2.3/install.ps1") {
      return new Response("Write-Host ok\n", { status: 200 });
    }
    throw new Error(`unexpected fetch: ${url}`);
  };

  try {
    const pending = [];
    const response = await worker.fetch(
      new Request("https://installer.example.test/install.ps1"),
      {},
      { waitUntil: (promise) => pending.push(promise) },
    );
    await Promise.all(pending);

    assert.equal(response.status, 200);
    assert.match(response.headers.get("content-type"), /text\/plain/);
    assert.match(await response.text(), /Write-Host ok/);
    assert.ok(
      calls.includes("https://raw.githubusercontent.com/OwnAnAI/ownhub/v1.2.3/install.ps1"),
    );
  } finally {
    globalThis.fetch = originalFetch;
    globalThis.caches = originalCaches;
  }
});

test("installer worker refuses paths it does not mirror", async () => {
  for (const path of ["/install.bat", "/install", "/"]) {
    const response = await worker.fetch(
      new Request(`https://installer.example.test${path}`),
      {},
      {},
    );
    if (path === "/") {
      assert.equal(response.status, 302);
      continue;
    }
    assert.equal(response.status, 404);
  }
});
