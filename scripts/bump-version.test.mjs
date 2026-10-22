/**
 * `bump-version.mjs` keeps package.json, tauri.conf.json and Cargo.toml on
 * the same release number. These tests pin the parsing, the bump rewrite
 * and the mismatch report — including the trap this script exists to avoid:
 * Cargo.toml dependency `version = "…"` lines must never be rewritten.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  assertVersion,
  bumpVersion,
  checkVersions,
  getCargoVersion,
  getJsonVersion,
  setCargoVersion,
  setJsonVersion,
} from "./bump-version.mjs";

const PACKAGE = `{
  "name": "orquestra-ai",
  "version": "0.1.0-alpha.3"
}
`;

const CARGO = `[package]
name = "orquestra-ai"
version = "0.1.0-alpha.3"

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
`;

test("semver accepts releases and prereleases, rejects the rest", () => {
  assert.equal(assertVersion("0.2.0"), "0.2.0");
  assert.equal(assertVersion("1.10.0-alpha.10"), "1.10.0-alpha.10");
  assert.throws(() => assertVersion("v1.2.3"), /semver/);
  assert.throws(() => assertVersion("1.2"), /semver/);
  assert.throws(() => assertVersion("latest"), /semver/);
  assert.throws(() => assertVersion(""), /semver/);
});

test("json round-trip keeps the payload, swaps only the version", () => {
  assert.equal(getJsonVersion(PACKAGE, "package.json"), "0.1.0-alpha.3");
  const next = setJsonVersion(PACKAGE, "0.2.0");
  assert.equal(getJsonVersion(next, "package.json"), "0.2.0");
  assert.match(next, /"name": "orquestra-ai"/);
  assert.throws(() => getJsonVersion("{}", "package.json"), /no string "version"/);
  assert.throws(() => getJsonVersion("{oops", "package.json"), /not valid JSON/);
});

test("cargo parsing stays inside [package], never touches dependencies", () => {
  assert.equal(getCargoVersion(CARGO), "0.1.0-alpha.3");
  const next = setCargoVersion(CARGO, "0.2.0");
  assert.equal(getCargoVersion(next), "0.2.0");
  assert.match(next, /tauri = \{ version = "2"/);
  assert.match(next, /serde = \{ version = "1"/);
  assert.throws(() => getCargoVersion("[dependencies]\n"), /no \[package\]/);
  assert.throws(() => setCargoVersion('[package]\nname = "x"\n', "0.2.0"), /no version line/);
});

function sandbox() {
  const root = mkdtempSync(join(tmpdir(), "bump-version-"));
  mkdirSync(join(root, "src-tauri"), { recursive: true });
  writeFileSync(join(root, "package.json"), PACKAGE);
  writeFileSync(join(root, "src-tauri/tauri.conf.json"), PACKAGE);
  writeFileSync(join(root, "src-tauri/Cargo.toml"), CARGO);
  return root;
}

test("check passes when the three manifests agree", () => {
  const { ok, entries } = checkVersions(sandbox());
  assert.equal(ok, true);
  assert.deepEqual(
    entries.map((entry) => entry.version),
    ["0.1.0-alpha.3", "0.1.0-alpha.3", "0.1.0-alpha.3"],
  );
});

test("check fails when one manifest drifts", () => {
  const root = sandbox();
  writeFileSync(join(root, "package.json"), setJsonVersion(PACKAGE, "0.9.9"));
  const { ok, entries } = checkVersions(root);
  assert.equal(ok, false);
  assert.equal(entries.find((entry) => entry.file === "package.json").version, "0.9.9");
});

test("bump rewrites all three manifests to the new version", () => {
  const root = sandbox();
  bumpVersion(root, "0.2.0");
  const { ok, entries } = checkVersions(root);
  assert.equal(ok, true);
  assert.ok(entries.every((entry) => entry.version === "0.2.0"));
  const cargo = readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8");
  assert.match(cargo, /^version = "0\.2\.0"$/m);
  assert.match(cargo, /tauri = \{ version = "2"/);
});

test("bump refuses invalid versions and broken manifests before writing", () => {
  const root = sandbox();
  assert.throws(() => bumpVersion(root, "nope"), /semver/);
  writeFileSync(join(root, "src-tauri/Cargo.toml"), "not toml at all\n");
  assert.throws(() => bumpVersion(root, "0.2.0"), /no \[package\]/);
  // Nothing was written: package.json still carries the old version.
  assert.equal(
    getJsonVersion(readFileSync(join(root, "package.json"), "utf8"), "x"),
    "0.1.0-alpha.3",
  );
});
