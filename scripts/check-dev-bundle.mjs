#!/usr/bin/env node
/**
 * Bundle guard: developer fixtures must not reach `dist/`.
 *
 * The fixtures carry absolute paths from a developer machine
 * (`/home/operator/...`, `/usr/local/bin/...`) and a full mock dataset. They
 * are loaded through `devFixtureLoader`, whose `import.meta.env.DEV` branch is
 * folded away in a production build; this check proves the chunk really is
 * gone instead of trusting the bundler.
 *
 * Runs after `vite build` (see package.json), so a regression fails the build
 * rather than shipping.
 */
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const DIST = resolve(fileURLToPath(new URL("../dist", import.meta.url)));

/** Strings that only exist in the mock fixtures. */
const MARKERS = ["home/operator", "transcriptFixtures", "mcpFixtures"];

function javascriptFiles(directory) {
  if (!existsSync(directory)) return [];
  const found = [];
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) {
      found.push(...javascriptFiles(path));
    } else if (entry.endsWith(".js")) {
      found.push(path);
    }
  }
  return found;
}

if (!existsSync(DIST)) {
  console.error("check-dev-bundle: dist/ is missing; run `vite build` first.");
  process.exit(1);
}

const leaks = [];
for (const file of javascriptFiles(DIST)) {
  const source = readFileSync(file, "utf8");
  for (const marker of MARKERS) {
    if (source.includes(marker)) {
      leaks.push(`${file.replace(DIST + "/", "")} contains ${marker}`);
    }
  }
}

if (leaks.length > 0) {
  console.error("check-dev-bundle: developer fixtures leaked into the build:");
  for (const leak of leaks) console.error(`  ${leak}`);
  process.exit(1);
}

console.log("✓ Production bundle contains no developer fixtures.");
