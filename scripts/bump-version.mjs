#!/usr/bin/env node
/**
 * Single-command version bumps across the three manifests that each carry
 * the release number: `package.json`, `src-tauri/tauri.conf.json` and
 * `src-tauri/Cargo.toml`. Manual edits kept drifting them apart, so:
 *
 * - `node scripts/bump-version.mjs <new-version>` rewrites all three
 *   (nothing is written unless every file parses and the version is valid
 *   semver, and each write goes through a temp file + rename so a crash
 *   never leaves a half-written manifest);
 * - `node scripts/bump-version.mjs --check` exits 0 when the three agree
 *   and 1 with a diff-style report when they do not (wired into CI).
 *
 * Run: `npm run version:bump -- 0.2.0` / `npm run version:check`.
 */
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(fileURLToPath(new URL("..", import.meta.url)));

export const MANIFESTS = ["package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml"];

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

/** Throw unless `version` is a release-shaped semver string. */
export function assertVersion(version) {
  if (typeof version !== "string" || !SEMVER.test(version)) {
    throw new Error(
      `invalid version ${JSON.stringify(version)}: expected semver like 0.2.0 or 0.2.0-alpha.1`,
    );
  }
  return version;
}

/** Extract `"version"` from a JSON manifest. */
export function getJsonVersion(text, file) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error(`${file} is not valid JSON`);
  }
  if (!parsed || typeof parsed.version !== "string") {
    throw new Error(`${file} has no string "version" field`);
  }
  return parsed.version;
}

/** Return the JSON text with `"version"` replaced (2-space style + newline). */
export function setJsonVersion(text, version) {
  const parsed = JSON.parse(text);
  parsed.version = version;
  return `${JSON.stringify(parsed, null, 2)}\n`;
}

/**
 * Extract `version = "…"` from the `[package]` section of Cargo.toml.
 * Dependency `version = "…"` lines elsewhere in the file are ignored by
 * scoping the search between `[package]` and the next section header.
 */
export function getCargoVersion(text) {
  const lines = text.split("\n");
  const start = lines.findIndex((line) => line.trim() === "[package]");
  if (start === -1) throw new Error("Cargo.toml has no [package] section");
  for (let index = start + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (/^\[.*\]/.test(line.trim())) break;
    const match = /^version\s*=\s*"([^"]*)"/.exec(line.trim());
    if (match) return match[1];
  }
  throw new Error("Cargo.toml [package] has no version line");
}

/** Return the Cargo.toml text with only the `[package]` version replaced. */
export function setCargoVersion(text, version) {
  const lines = text.split("\n");
  const start = lines.findIndex((line) => line.trim() === "[package]");
  if (start === -1) throw new Error("Cargo.toml has no [package] section");
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^\[.*\]/.test(lines[index].trim())) break;
    if (/^version\s*=\s*"[^"]*"/.test(lines[index].trim())) {
      lines[index] = `version = "${version}"`;
      return lines.join("\n");
    }
  }
  throw new Error("Cargo.toml [package] has no version line");
}

const isCargo = (file) => file.endsWith("Cargo.toml");

export function readVersion(file, root = ROOT) {
  const text = readFileSync(join(root, file), "utf8");
  return isCargo(file) ? getCargoVersion(text) : getJsonVersion(text, file);
}

/** Read every manifest version: `[{ file, version }]`. Throws on any problem. */
export function readVersions(root = ROOT) {
  return MANIFESTS.map((file) => ({ file, version: readVersion(file, root) }));
}

/**
 * Compare the three manifests. Returns `{ ok, entries }`; `ok` is false
 * when any two disagree (or any file is unreadable/invalid).
 */
export function checkVersions(root = ROOT) {
  let entries;
  try {
    entries = readVersions(root);
  } catch (error) {
    return { ok: false, entries: [], error: error.message };
  }
  const distinct = new Set(entries.map((entry) => entry.version));
  return { ok: distinct.size === 1, entries };
}

/**
 * Rewrite all three manifests to `version`. Every file is parsed first, so
 * a broken manifest aborts before anything is written; each write lands via
 * temp file + rename so a crash never leaves a half-written manifest.
 */
export function bumpVersion(root, version) {
  assertVersion(version);
  const pending = MANIFESTS.map((file) => {
    const path = join(root, file);
    const text = readFileSync(path, "utf8");
    const next = isCargo(file) ? setCargoVersion(text, version) : setJsonVersion(text, version);
    return { path, next };
  });
  for (const { path, next } of pending) {
    const tmp = `${path}.version-tmp`;
    writeFileSync(tmp, next, "utf8");
    renameSync(tmp, path);
  }
  return MANIFESTS.map((file) => ({ file, version }));
}

const usage = "usage: bump-version.mjs <new-version> | bump-version.mjs --check";

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [arg] = process.argv.slice(2);
  if (arg === "--check") {
    const { ok, entries, error } = checkVersions();
    if (!ok) {
      console.error("✖ version: manifests disagree");
      for (const { file, version } of entries) console.error(`  ${file}: ${version}`);
      if (error) console.error(`  ${error}`);
      process.exit(1);
    }
    console.log(`✔ version: ${MANIFESTS.length} manifests agree on ${entries[0].version}.`);
  } else if (arg) {
    try {
      const entries = bumpVersion(ROOT, arg);
      console.log(`✔ version: bumped ${entries.length} manifests to ${arg}.`);
    } catch (error) {
      console.error(`✖ version: ${error.message}`);
      process.exit(1);
    }
  } else {
    console.error(usage);
    process.exit(1);
  }
}
