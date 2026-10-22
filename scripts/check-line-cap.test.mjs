/**
 * `check-line-cap.mjs` is the ratchet that keeps this repo's splits from
 * regrowing. These tests pin the counting, the ratchet's four verdicts
 * (new offender, growth, shrink, clean) and the live tree, so the check cannot
 * be quietly loosened into a no-op.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import {
  BASELINE_FILE,
  LINE_CAP,
  countLines,
  deriveBaseline,
  inspect,
  loadBaseline,
  staleEntries,
  violations,
} from "./check-line-cap.mjs";

/** A file of exactly `lines` lines, newline-terminated like the real tree. */
function source(lines) {
  return "// filler\n".repeat(lines);
}

/** Build a throwaway tree from `{ path: lineCount }`. */
function tree(files) {
  const root = mkdtempSync(join(tmpdir(), "orquestra-line-cap-"));
  for (const [path, lines] of Object.entries(files)) {
    const full = join(root, path);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, source(lines));
  }
  return root;
}

test("countLines matches what an editor shows", () => {
  assert.equal(countLines(""), 0);
  assert.equal(countLines("one\n"), 1);
  assert.equal(countLines("one\ntwo\n"), 2);
  assert.equal(countLines("one\ntwo"), 2, "a missing final newline still ends the line");
  assert.equal(countLines("one\n\n"), 2, "a trailing blank line is not an extra line");
});

test("only the cap and the baseline decide what fails", () => {
  const root = tree({
    "src/at_the_cap.rs": LINE_CAP,
    "src/over.rs": LINE_CAP + 1,
    "src/baselined.rs": 900,
    "src/baselined_and_shortened.rs": 600,
    "src/ignored/target/generated.rs": 5000,
    "src/ignored/node_modules/vendored.rs": 5000,
    "src/notes.txt": 900,
  });
  try {
    const files = inspect(root);
    assert.deepEqual(
      files.map((entry) => entry.path),
      ["src/at_the_cap.rs", "src/baselined.rs", "src/baselined_and_shortened.rs", "src/over.rs"],
      "build output, vendored trees and non-Rust files are skipped",
    );

    const baseline = { "src/baselined.rs": 900, "src/baselined_and_shortened.rs": 700 };
    const found = violations(files, baseline).map((problem) => problem.path);
    assert.deepEqual(found, ["src/baselined_and_shortened.rs", "src/over.rs"]);

    assert.deepEqual(
      violations(files, {
        "src/baselined.rs": 900,
        "src/baselined_and_shortened.rs": 600,
        "src/over.rs": 401,
      }),
      [],
      "a file may sit at the cap, and a baselined file may stay put",
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("growth past a recorded length is the regression this guards", () => {
  const files = [{ path: "src/a.rs", lines: 401 }];
  assert.equal(violations(files, { "src/a.rs": 400 }).length, 1);
  assert.equal(violations(files, { "src/a.rs": 401 }).length, 0);
});

test("stale entries are reported without failing the run", () => {
  assert.deepEqual(staleEntries([{ path: "src/a.rs", lines: 10 }], { "src/gone.rs": 900 }), [
    "src/gone.rs",
  ]);
});

test("deriveBaseline keeps exactly the files over the cap", () => {
  const baseline = deriveBaseline([
    { path: "src/small.rs", lines: 40 },
    { path: "src/big.rs", lines: 401 },
  ]);
  assert.deepEqual(baseline, { "src/big.rs": 401 });
});

test("this workspace passes its own ratchet", () => {
  const problems = violations(inspect(), loadBaseline());
  assert.deepEqual(
    problems,
    [],
    `run \`npm run check:lines -- --update\` only for an intended length change:\n${JSON.stringify(problems, null, 2)}`,
  );
  assert.ok(BASELINE_FILE.endsWith(".json"));
});
