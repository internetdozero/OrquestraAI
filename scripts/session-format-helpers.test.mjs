/**
 * Unit tests for `src/lib/sessionFormat.ts`.
 *
 * Asserts formatting logic for byte sizes, dates, relative age, paths,
 * and harness color badges.
 *
 * Run: `npm test`
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  absoluteDate,
  formatBytes,
  formatDateTime,
  formatMb,
  harnessColor,
  relativeAge,
  shortPath,
} from "../src/lib/sessionFormat.ts";

test("harnessColor returns expected CSS badge classes for known harnesses", () => {
  const expectedColors = {
    antigravity: "text-violet-300 border-violet-500/25 bg-violet-500/10",
    codex: "text-emerald-300 border-emerald-500/25 bg-emerald-500/10",
    claude: "text-orange-300 border-orange-500/25 bg-orange-500/10",
    freebuff: "text-cyan-300 border-cyan-500/25 bg-cyan-500/10",
    opencode: "text-sky-300 border-sky-500/25 bg-sky-500/10",
    kilo: "text-teal-300 border-teal-500/25 bg-teal-500/10",
    copilot: "text-blue-300 border-blue-500/25 bg-blue-500/10",
    hermes: "text-fuchsia-300 border-fuchsia-500/25 bg-fuchsia-500/10",
    grok: "text-zinc-200 border-white/15 bg-white/[0.05]",
  };

  for (const [harness, expectedClass] of Object.entries(expectedColors)) {
    assert.equal(
      harnessColor(harness),
      expectedClass,
      `Incorrect badge style for harness: ${harness}`,
    );
  }
});

test("harnessColor returns fallback default class for unknown harnesses or empty strings", () => {
  const fallback = "text-zinc-300 border-white/10 bg-white/[0.04]";
  assert.equal(harnessColor("unknown-harness"), fallback);
  assert.equal(harnessColor(""), fallback);
  assert.equal(harnessColor("custom"), fallback);
});

test("formatBytes formats bytes into 1024-based human readable strings", () => {
  assert.equal(formatBytes(NaN), "0 B");
  assert.equal(formatBytes(-100), "0 B");
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(1024), "1.0 KB");
  assert.equal(formatBytes(1536), "1.5 KB");
  assert.equal(formatBytes(120 * 1024), "120 KB");
  assert.equal(formatBytes(1024 * 1024 * 2.5), "2.5 MB");
  assert.equal(formatBytes(1024 * 1024 * 1024 * 3), "3.0 GB");
});

test("formatMb formats byte sizes to megabytes short string", () => {
  assert.equal(formatMb(1024 * 1024 * 12.4), "12.4 MB");
  assert.equal(formatMb(0), "0.0 MB");
});

test("shortPath shortens paths to last two segments or fallback", () => {
  assert.equal(shortPath(null), "—");
  assert.equal(shortPath(""), "—");
  assert.equal(shortPath("src"), "src");
  assert.equal(shortPath("src/lib"), "src/lib");
  assert.equal(shortPath("/a/b/c/d"), "…/c/d");
  assert.equal(shortPath("C:\\Users\\Dev\\Project\\file.ts"), "…/Project/file.ts");
});

test("relativeAge formats ISO date relative to current time in en/pt locales", () => {
  assert.equal(relativeAge("not-a-date", "en"), "not-a-date");

  const now = Date.now();
  const todayIso = new Date(now).toISOString();
  assert.equal(relativeAge(todayIso, "en"), "today");
  assert.equal(relativeAge(todayIso, "pt"), "hoje");

  const oneDayAgoIso = new Date(now - 86400000).toISOString();
  assert.equal(relativeAge(oneDayAgoIso, "en"), "1d ago");
  assert.equal(relativeAge(oneDayAgoIso, "pt"), "há 1 dia");

  const fiveDaysAgoIso = new Date(now - 5 * 86400000).toISOString();
  assert.equal(relativeAge(fiveDaysAgoIso, "en"), "5d ago");
  assert.equal(relativeAge(fiveDaysAgoIso, "pt"), "há 5 dias");
});

test("formatDateTime and absoluteDate format valid ISO dates and handle invalid inputs", () => {
  assert.equal(formatDateTime("invalid-date"), "invalid-date");
  assert.equal(absoluteDate("invalid-date", "en"), "invalid-date");

  const iso = "2026-10-18T05:41:00.000Z";
  const formatted = formatDateTime(iso);
  assert.ok(typeof formatted === "string" && formatted.includes(":"));

  const absEn = absoluteDate(iso, "en");
  assert.ok(typeof absEn === "string" && absEn.length > 0);
  const absPt = absoluteDate(iso, "pt");
  assert.ok(typeof absPt === "string" && absPt.length > 0);
});
