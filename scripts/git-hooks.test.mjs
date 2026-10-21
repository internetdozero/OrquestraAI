/**
 * Tests for the versioned git hooks wiring.
 *
 * The card-motion guardrail only protects a commit if the hook exists, is
 * executable, and `core.hooksPath` points at this directory. Deleting the hook
 * or dropping the `prepare` script should fail here instead of quietly letting
 * `transition-all` back into a card.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const HOOK = resolve(ROOT, ".githooks/pre-commit");

test("the pre-commit hook is executable and gates staged sources", () => {
  const mode = statSync(HOOK).mode;
  assert.ok(mode & 0o111, "pre-commit must stay executable to run under git");

  const body = readFileSync(HOOK, "utf8");
  assert.match(body, /check-card-motion\.mjs/, "hook must run the card-motion guard");
  assert.match(
    body,
    /-- '\*\.ts' '\*\.tsx' '\*\.css'/,
    "hook must run whenever TS, TSX or CSS is staged",
  );
});

test("npm install points core.hooksPath at the versioned hooks", () => {
  const { scripts } = JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf8"));
  assert.equal(scripts.prepare, "git config core.hooksPath .githooks || true");
  assert.equal(scripts["hooks:install"], "git config core.hooksPath .githooks");
});
