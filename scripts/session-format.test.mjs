/**
 * Tests for `formatBytes` in `src/lib/sessionFormat.ts`.
 *
 * Ensures human-readable 1024-based size formatting across byte, KB, MB, GB,
 * and TB units, with proper handling of invalid/negative inputs and unit boundaries.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { formatBytes } from "../src/lib/sessionFormat.ts";

test("returns '0 B' for invalid or negative inputs", () => {
  assert.equal(formatBytes(NaN), "0 B");
  assert.equal(formatBytes(Infinity), "0 B");
  assert.equal(formatBytes(-Infinity), "0 B");
  assert.equal(formatBytes(-1), "0 B");
  assert.equal(formatBytes(-1024), "0 B");
});

test("formats byte values under 1024 directly as 'N B'", () => {
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(1), "1 B");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(1023), "1023 B");
});

test("formats KB sizes with one decimal for <100 and rounded integer for >=100", () => {
  assert.equal(formatBytes(1024), "1.0 KB");
  assert.equal(formatBytes(1536), "1.5 KB");
  assert.equal(formatBytes(99.9 * 1024), "99.9 KB");
  assert.equal(formatBytes(100 * 1024), "100 KB");
  assert.equal(formatBytes(1023 * 1024), "1023 KB");
});

test("formats MB sizes correctly", () => {
  assert.equal(formatBytes(1024 * 1024), "1.0 MB");
  assert.equal(formatBytes(14.5 * 1024 * 1024), "14.5 MB");
  assert.equal(formatBytes(100 * 1024 * 1024), "100 MB");
});

test("formats GB sizes correctly", () => {
  assert.equal(formatBytes(2.4 * 1024 * 1024 * 1024), "2.4 GB");
  assert.equal(formatBytes(150 * 1024 * 1024 * 1024), "150 GB");
});

test("formats TB sizes and caps unit at TB for larger inputs", () => {
  assert.equal(formatBytes(1024 * 1024 * 1024 * 1024), "1.0 TB");
  assert.equal(formatBytes(100 * 1024 * 1024 * 1024 * 1024), "100 TB");
  assert.equal(formatBytes(5000 * 1024 * 1024 * 1024 * 1024), "5000 TB");
});
