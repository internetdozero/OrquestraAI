#!/usr/bin/env node
/**
 * Line-cap guard.
 *
 * AGENTS.md keeps every file under 300–400 lines, because a 900-line module is
 * where "just one more case in here" hides. Nothing enforced it, so splits
 * regrew quietly: this check is the ratchet.
 *
 * Seven Rust files are already over the cap. Instead of failing on them
 * forever — or waving a blanket exemption — the baseline records exactly how
 * long each one is today:
 *
 * - a file that is not in the baseline fails as soon as it passes the cap;
 * - a baselined file fails on *any* change to its length, growth because it
 *   regressed, shrinking because the entry should have been lowered or dropped.
 *
 * Run `npm run check:lines` to check, `npm run check:lines -- --update` after a
 * deliberate split to rewrite the entries (the diff is the review).
 */
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Hard ceiling: a file of this length is over the documented cap. */
export const LINE_CAP = 400;

export const BASELINE_FILE = "scripts/line-cap-baseline.json";

const ROOT = resolve(fileURLToPath(new URL("..", import.meta.url)));

/** Build output and vendored trees are not source we maintain. */
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

/** Every Rust source and test file in the workspace, workspace-relative. */
export function rustFiles(root = ROOT) {
  const found = [];
  const walk = (relative) => {
    const entries = readdirSync(join(root, relative), { withFileTypes: true });
    for (const entry of entries) {
      const path = relative === "" ? entry.name : `${relative}/${entry.name}`;
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) walk(path);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        found.push(path);
      }
    }
  };

  walk("");
  return found.sort();
}

/**
 * Lines in `content`, counted the way an editor shows them: a trailing newline
 * ends the last line instead of starting an empty one, and an empty file is 0.
 */
export function countLines(content) {
  if (content === "") return 0;
  const lines = content.split("\n");
  return lines.at(-1) === "" ? lines.length - 1 : lines.length;
}

/** `{ path, lines }` for every Rust file, sorted by path. */
export function inspect(root = ROOT) {
  return rustFiles(root).map((path) => ({
    path,
    lines: countLines(readFileSync(join(root, path), "utf8")),
  }));
}

export function loadBaseline(root = ROOT) {
  const path = join(root, BASELINE_FILE);
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : {};
}

/**
 * Files that break the ratchet: newly over the cap, longer than their entry,
 * or shorter than it (the entry is now wrong and should be lowered).
 */
export function violations(files, baseline) {
  const problems = [];
  for (const { path, lines } of files) {
    const recorded = baseline[path];
    if (recorded === undefined) {
      if (lines > LINE_CAP) {
        problems.push({
          path,
          lines,
          reason: `is over the ${LINE_CAP}-line cap and not in the baseline`,
        });
      }
    } else if (lines > recorded) {
      problems.push({
        path,
        lines,
        reason: `grew from its baselined ${recorded} lines`,
      });
    } else if (lines < recorded) {
      problems.push({
        path,
        lines,
        reason: `shrank from its baselined ${recorded} lines — lower or drop the entry`,
      });
    }
  }

  return problems;
}

/** Baseline entries whose file no longer exists, for the report only. */
export function staleEntries(files, baseline) {
  const present = new Set(files.map((entry) => entry.path));
  return Object.keys(baseline)
    .filter((path) => !present.has(path))
    .sort();
}

/** The baseline these files imply: only files over the cap keep an entry. */
export function deriveBaseline(files) {
  const baseline = {};
  for (const { path, lines } of files) {
    if (lines > LINE_CAP) baseline[path] = lines;
  }
  return baseline;
}

function main() {
  const files = inspect();
  const update = process.argv.includes("--update");

  if (update) {
    const baseline = deriveBaseline(files);
    writeFileSync(join(ROOT, BASELINE_FILE), `${JSON.stringify(baseline, null, 2)}\n`);
    console.log(`line-cap: baseline rewritten with ${Object.keys(baseline).length} entries`);
    return 0;
  }

  const baseline = loadBaseline();
  const problems = violations(files, baseline);
  const stale = staleEntries(files, baseline);

  for (const problem of problems) {
    console.error(`line-cap: ${problem.path} ${problem.reason} (now ${problem.lines})`);
  }
  for (const path of stale) {
    console.warn(`line-cap: ${path} is baselined but no longer exists`);
  }

  if (problems.length > 0) {
    console.error(
      `line-cap: ${problems.length} violation(s); split the file, or run \`npm run check:lines -- --update\` if the new length is intended.`,
    );
    return 1;
  }

  console.log(
    `line-cap: ${files.length} Rust files under the ${LINE_CAP}-line cap except ${Object.keys(baseline).length} baselined`,
  );
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main();
}
