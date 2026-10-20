/**
 * Release checks go through `checkLatestRelease` in `src/lib/updater.ts`.
 * The running package is `0.1.0-alpha.3`; a comparator that splits on `.`
 * treats that pre-release as newer than `0.1.0` and hides the stable tag.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { checkLatestRelease } from "../src/lib/updater.ts";

async function withTag(tag, current) {
  const original = globalThis.fetch;
  globalThis.fetch = async (url) => {
    assert.match(String(url), /\/releases\/latest$/);
    return new Response(
      JSON.stringify({ tag_name: tag, html_url: "https://example.test/release" }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    );
  };
  try {
    return await checkLatestRelease(current);
  } finally {
    globalThis.fetch = original;
  }
}

test("the alpha build is behind the same stable release", async () => {
  const result = await withTag("v0.1.0", "0.1.0-alpha.3");
  assert.equal(result.isLatest, false);
  assert.equal(result.latestVersion, "0.1.0");
  assert.equal(result.releaseUrl, "https://example.test/release");
});

test("numeric ordering and newer pre-releases still move forward", async () => {
  const same = await withTag("v0.1.0-alpha.3", "0.1.0-alpha.3");
  assert.equal(same.isLatest, true);
  assert.equal(same.latestVersion, "0.1.0-alpha.3");

  const olderPre = await withTag("v0.1.0-alpha.2", "0.1.0-alpha.3");
  assert.equal(olderPre.isLatest, true);

  const newerPre = await withTag("v0.1.0-alpha.10", "0.1.0-alpha.3");
  assert.equal(newerPre.isLatest, false);
  assert.equal(newerPre.latestVersion, "0.1.0-alpha.10");

  const minor = await withTag("v1.10.0", "1.9.0");
  assert.equal(minor.isLatest, false);
  assert.equal(minor.latestVersion, "1.10.0");
});

test("a failed release lookup does not invent an update", async () => {
  const original = globalThis.fetch;
  globalThis.fetch = async () => {
    throw new Error("offline");
  };
  try {
    const result = await checkLatestRelease("0.1.0-alpha.3");
    assert.equal(result.isLatest, true);
    assert.equal(result.latestVersion, "0.1.0-alpha.3");
  } finally {
    globalThis.fetch = original;
  }
});
