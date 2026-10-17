#!/usr/bin/env node
/**
 * Card-motion regression guard.
 *
 * The skill and MCP grids once jittered violently under the cursor because a
 * card used `transition-all` plus a hover lift. `transition-all` re-lays-out
 * every card on each hover frame, and a hover transform slides the hit area
 * out from under the pointer, which cancels the hover and instantly re-applies
 * it — a 60fps flicker loop. Both bugs are invisible in a unit test, so this
 * script scans the rendered JSX for the two class patterns that cause them.
 *
 * Run: `npm run check:ui` (also wired into `npm run build`).
 */
import { globSync, readFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(fileURLToPath(new URL("..", import.meta.url)));

const RULES = [
  {
    id: "no-transition-all",
    pattern: /\btransition-(?:all|\[all\])/,
    hint: "list paint properties instead: transition-[border-color,background-color,box-shadow]",
  },
  {
    id: "no-hover-geometry",
    pattern: /\bhover:-?(?:translate-[xy]|rotate)-/,
    hint: "emphasize with border/shadow instead of moving the element",
  },
];

/** Card roots must keep their repaints local (`[contain:content]`). */
const CARD_FILE = /(^|\/)components\/[^/]*Card\.tsx$/;
const CONTAINMENT = /\bcontain\s*:/;

/**
 * Blank out `//` and block comments so prose about the old bug (this file's
 * own subjects are described in comments all over the components) is never
 * mistaken for code. Line numbering is preserved: each line is handled alone.
 */
function stripComments(line, state) {
  let out = "";
  let index = 0;

  while (index < line.length) {
    if (state.inBlock) {
      const end = line.indexOf("*/", index);
      if (end === -1) return out;
      state.inBlock = false;
      index = end + 2;
      continue;
    }

    if (line.startsWith("/*", index)) {
      state.inBlock = true;
      index += 2;
      continue;
    }

    if (line.startsWith("//", index)) return out;

    out += line[index];
    index += 1;
  }

  return out;
}

function checkFile(absolutePath) {
  const path = relative(ROOT, absolutePath);
  const lines = readFileSync(absolutePath, "utf8").split("\n");
  const state = { inBlock: false };
  const findings = [];
  let source = "";

  lines.forEach((line, index) => {
    const code = stripComments(line, state);
    source += `${code}\n`;

    for (const rule of RULES) {
      if (rule.pattern.test(code)) {
        findings.push({ path, line: index + 1, rule, text: line.trim() });
      }
    }
  });

  if (CARD_FILE.test(path) && !CONTAINMENT.test(source)) {
    findings.push({
      path,
      line: 1,
      rule: {
        id: "card-keeps-containment",
        hint: "add [contain:content] to the card root so hover/flash repaints stay local",
      },
      text: "no CSS containment on the card root",
    });
  }

  return findings;
}

// Paths are resolved against the repo root, never the caller's cwd: Tauri runs
// `beforeBuildCommand` from its own working directory.
const files = globSync("src/**/*.tsx", { cwd: ROOT }).map((entry) =>
  resolve(ROOT, entry),
);
const findings = files.flatMap(checkFile);

if (findings.length === 0) {
  console.log(
    `✔ ui-motion: ${files.length} TSX files checked, no card-motion regressions.`,
  );
  process.exit(0);
}

console.error(`✖ ui-motion: ${findings.length} card-motion regression(s)\n`);
for (const { path, line, rule, text } of findings) {
  console.error(`  ${path}:${line}  [${rule.id}]`);
  console.error(`    ${text}`);
  console.error(`    → ${rule.hint}\n`);
}

process.exit(1);
