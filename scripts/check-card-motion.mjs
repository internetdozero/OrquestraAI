#!/usr/bin/env node
/**
 * Card-motion regression guard.
 *
 * The skill and MCP grids once jittered violently under the cursor because a
 * card used `transition-all` plus a hover lift. `transition-all` re-lays-out
 * every card on each hover frame, and a hover transform slides the hit area
 * out from under the pointer, which cancels the hover and instantly re-applies
 * it — a 60fps flicker loop. Both bugs are invisible in a unit test, so this
 * script scans the sources for the patterns that cause them:
 *
 * - TS/TSX class strings (`transition-all`, `hover:translate-y-*`);
 * - TS/TSX inline style objects (`transition: "all …"`, `transitionProperty`);
 * - CSS declarations (`transition: all`, `transition-property: all`) and any
 *   transform inside a `:hover` block, where the selector sits on an earlier
 *   line than the declaration that moves the element.
 *
 * Run: `npm run check:ui` (also wired into `npm run build` and pre-commit).
 */
import { globSync, readFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(fileURLToPath(new URL("..", import.meta.url)));

const TRANSITION_ALL = {
  id: "no-transition-all",
  pattern: /\btransition-(?:all|\[all\])/,
  hint: "list paint properties instead: transition-[border-color,background-color,box-shadow]",
};

const HOVER_GEOMETRY = {
  id: "no-hover-geometry",
  pattern: /\bhover:-?(?:translate-[xy]|rotate)-/,
  hint: "emphasize with border/shadow instead of moving the element",
};

/** Rules applied line by line to every TS/TSX file. */
const SOURCE_RULES = [
  TRANSITION_ALL,
  HOVER_GEOMETRY,
  {
    id: "no-inline-transition-all",
    pattern: /\btransition(?:Property)?\s*:\s*["'`][^"'`]*\ball\b/,
    hint: "inline style objects must list paint properties instead of 'all'",
  },
];

/** Rules applied line by line to every stylesheet. */
const STYLE_RULES = [
  TRANSITION_ALL,
  {
    id: "no-css-transition-all",
    pattern: /(?:^|[;{\s])transition(?:-property)?\s*:[^;{}]*\ball\b/,
    hint: "transition only the paint properties that actually change",
  },
];

/** A declaration that moves or resizes an element (inside a `:hover` block).
 * Tailwind v4 emits the individual `translate`/`scale` properties, so a plain
 * numeric value counts as geometry; `none` and friends stay harmless. */
const CSS_GEOMETRY = /\b(?:transform|translate|rotate|scale)\s*:\s*([^;{}]+)/;
const CSS_NO_OP = /^(?:none|initial|inherit|unset|revert|revert-layer)$/i;

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

function checkSource(absolutePath) {
  const path = relative(ROOT, absolutePath);
  const lines = readFileSync(absolutePath, "utf8").split("\n");
  const state = { inBlock: false };
  const findings = [];
  let source = "";

  lines.forEach((line, index) => {
    const code = stripComments(line, state);
    source += `${code}\n`;

    for (const rule of SOURCE_RULES) {
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

/**
 * Stylesheets need brace tracking: the `:hover` sits on the selector line while
 * the declaration that moves the element comes later, inside the block. Blocks
 * nested in a hover rule count as hover too, so an `@media` inside one is safe.
 */
function checkStylesheet(absolutePath) {
  const path = relative(ROOT, absolutePath);
  const lines = readFileSync(absolutePath, "utf8").split("\n");
  const state = { inBlock: false };
  const hoverStack = [];
  const findings = [];
  let hoverDepth = 0;

  lines.forEach((line, index) => {
    const code = stripComments(line, state);

    for (const rule of STYLE_RULES) {
      if (rule.pattern.test(code)) {
        findings.push({ path, line: index + 1, rule, text: line.trim() });
      }
    }

    const geometry = CSS_GEOMETRY.exec(code);
    if (hoverDepth > 0 && geometry && !CSS_NO_OP.test(geometry[1].trim())) {
      findings.push({ path, line: index + 1, rule: HOVER_GEOMETRY, text: line.trim() });
    }

    const isHoverSelector = /:hover\b/.test(code);
    for (const character of code) {
      if (character === "{") {
        hoverStack.push(isHoverSelector);
        if (isHoverSelector) hoverDepth += 1;
      } else if (character === "}") {
        if (hoverStack.pop()) hoverDepth -= 1;
      }
    }
  });

  return findings;
}

// Paths are resolved against the repo root, never the caller's cwd: Tauri runs
// `beforeBuildCommand` from its own working directory.
const collect = (pattern) => globSync(pattern, { cwd: ROOT }).map((entry) => resolve(ROOT, entry));

const tsxFiles = collect("src/**/*.tsx");
const tsFiles = collect("src/**/*.ts");
const cssFiles = collect("src/**/*.css");

const findings = [
  ...tsxFiles.flatMap(checkSource),
  ...tsFiles.flatMap(checkSource),
  ...cssFiles.flatMap(checkStylesheet),
];

if (findings.length === 0) {
  console.log(
    `✔ ui-motion: ${tsxFiles.length} TSX + ${tsFiles.length} TS + ${cssFiles.length} CSS files checked, no card-motion regressions.`,
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
