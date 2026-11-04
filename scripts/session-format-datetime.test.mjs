/**
 * Tests for session formatting utilities in `src/lib/sessionFormat.ts`.
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

test("formatDateTime formats valid ISO strings with zero-padding (DD/MM/YYYY HH:mm)", () => {
  // Construct dates with specific local time components
  const d1 = new Date(2026, 0, 2, 3, 4); // Jan 2, 2026 03:04
  const iso1 = d1.toISOString();
  assert.equal(formatDateTime(iso1), `02/01/2026 03:04`);

  const d2 = new Date(2026, 10, 25, 18, 45); // Nov 25, 2026 18:45
  const iso2 = d2.toISOString();
  assert.equal(formatDateTime(iso2), `25/11/2026 18:45`);
});

test("formatDateTime returns original input for invalid date strings", () => {
  assert.equal(formatDateTime("invalid-date-string"), "invalid-date-string");
  assert.equal(formatDateTime(""), "");
  assert.equal(formatDateTime("not-a-date"), "not-a-date");
});

test("formatBytes formats byte counts in human readable units", () => {
  assert.equal(formatBytes(NaN), "0 B");
  assert.equal(formatBytes(-100), "0 B");
  assert.equal(formatBytes(Infinity), "0 B");
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(500), "500 B");
  assert.equal(formatBytes(1024), "1.0 KB");
  assert.equal(formatBytes(1024 * 1.5), "1.5 KB");
  assert.equal(formatBytes(1024 * 1024 * 2.4), "2.4 MB");
  assert.equal(formatBytes(1024 * 1024 * 150), "150 MB");
  assert.equal(formatBytes(1024 * 1024 * 1024 * 2.4), "2.4 GB");
  assert.equal(formatBytes(1024 * 1024 * 1024 * 1024 * 3), "3.0 TB");
});

test("formatMb formats byte counts into short MB string", () => {
  assert.equal(formatMb(0), "0.0 MB");
  assert.equal(formatMb(1024 * 1024 * 12.4), "12.4 MB");
});

test("relativeAge formats relative time in pt and en locales", () => {
  assert.equal(relativeAge("invalid-date", "pt"), "invalid-date");

  const now = Date.now();
  const dayMs = 86_400_000;

  const todayIso = new Date(now).toISOString();
  assert.equal(relativeAge(todayIso, "pt"), "hoje");
  assert.equal(relativeAge(todayIso, "en"), "today");

  const yesterdayIso = new Date(now - dayMs).toISOString();
  assert.equal(relativeAge(yesterdayIso, "pt"), "há 1 dia");
  assert.equal(relativeAge(yesterdayIso, "en"), "1d ago");

  const multiDaysIso = new Date(now - 18 * dayMs).toISOString();
  assert.equal(relativeAge(multiDaysIso, "pt"), "há 18 dias");
  assert.equal(relativeAge(multiDaysIso, "en"), "18d ago");
});

test("absoluteDate formats date according to locale or returns input for invalid date", () => {
  assert.equal(absoluteDate("invalid", "en"), "invalid");

  const iso = "2026-10-18T05:41:00.000Z";
  const formattedEn = absoluteDate(iso, "en");
  const formattedPt = absoluteDate(iso, "pt");

  assert.ok(typeof formattedEn === "string" && formattedEn.length > 0);
  assert.ok(typeof formattedPt === "string" && formattedPt.length > 0);
  assert.notEqual(formattedEn, "invalid");
});

test("shortPath shortens long workspace paths to last two segments", () => {
  assert.equal(shortPath(null), "—");
  assert.equal(shortPath(""), "—");
  assert.equal(shortPath("/a"), "/a");
  assert.equal(shortPath("/a/b"), "/a/b");
  assert.equal(shortPath("/a/b/c/d/e"), "…/d/e");
  assert.equal(shortPath("C:\\Users\\dev\\project\\src"), "…/project/src");
});

test("harnessColor returns expected Tailwind classes for known and unknown harnesses", () => {
  assert.ok(harnessColor("antigravity").includes("text-violet-300"));
  assert.ok(harnessColor("codex").includes("text-emerald-300"));
  assert.ok(harnessColor("claude").includes("text-orange-300"));
  assert.ok(harnessColor("freebuff").includes("text-cyan-300"));
  assert.ok(harnessColor("opencode").includes("text-sky-300"));
  assert.ok(harnessColor("kilo").includes("text-teal-300"));
  assert.ok(harnessColor("copilot").includes("text-blue-300"));
  assert.ok(harnessColor("hermes").includes("text-fuchsia-300"));
  assert.ok(harnessColor("grok").includes("text-zinc-200"));
  assert.ok(harnessColor("unknown").includes("text-zinc-300"));
});
