/**
 * Visual regression gate for the nine harness transcript drawers.
 *
 * Boots a throwaway Vite dev server (fixture mode), drives headless Chrome,
 * and snapshots each fixture drawer's rendered structure, text, alignment and
 * Markdown blocks. Snapshots are deterministic (no timestamps, locale or
 * pixels), so a changed parser, sanitizer or bubble layout fails the run.
 *
 *   npm run test:visual              compare against the golden file
 *   UPDATE_SNAPSHOTS=1 npm run test:visual   rewrite the golden file
 *
 * Chrome is located via CHROME_PATH or the usual system binaries.
 */
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { connectPage, launchChrome } from "./visual/cdp.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PORT = 1422;
const APP_URL = `http://127.0.0.1:${PORT}/?fixtures=1`;
const GOLDEN =
  process.env.SNAPSHOT_FILE || join(ROOT, "scripts/__snapshots__/transcript-drawers.json");
const TMP = process.env.ORQUESTRA_VISUAL_TMP || "/var/tmp/testing";
const HARNESSES = [
  "antigravity",
  "grok",
  "codex",
  "claude",
  "freebuff",
  "copilot",
  "hermes",
  "opencode",
  "kilo",
];
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

function startVite() {
  const child = spawn(npm, ["run", "dev", "--", "--port", String(PORT), "--strictPort"], {
    cwd: ROOT,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let log = "";
  child.stdout.on("data", (chunk) => {
    log += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    log += chunk.toString();
  });
  return { child, log: () => log };
}

async function waitForServer(vite) {
  const until = Date.now() + 40_000;
  while (Date.now() < until) {
    if (vite.child.exitCode !== null) {
      throw new Error(`Vite exited (${vite.child.exitCode}):\n${vite.log().slice(-2000)}`);
    }
    try {
      if ((await fetch(APP_URL)).ok) return;
    } catch {
      /* still booting */
    }
    await delay(150);
  }
  throw new Error(`Vite did not start on ${PORT}:\n${vite.log().slice(-2000)}`);
}

async function openSessionsList(page) {
  await page.waitFor("!!document.querySelector('aside')", "app shell");
  await page.evaluate(`(() => {
    [...document.querySelectorAll('aside button')]
      .find((button) => button.textContent.includes('Sessões') || button.textContent.includes('Sessions'))
      ?.click();
    return true;
  })()`);
  await page.waitFor(
    `document.querySelectorAll('[data-session-harness]').length >= ${HARNESSES.length}`,
    "nine fixture rows",
  );
}

const SNAPSHOT_EXPRESSION = (harness) => `(() => {
  const row = document.querySelector('[data-session-harness="${harness}"]');
  if (!row) return null;
  row.querySelector('button')?.click();
  return true;
})()`;

const READ_DRAWER = `(() => {
  const drawer = document.querySelector('[role=dialog]');
  if (!drawer) return null;
  const turns = [...drawer.querySelectorAll('[data-turn-role]')].map((turn) => {
    const bubble = turn.firstElementChild;
    const content = bubble.querySelector('.selectable-text');
    const inlineCode = [...content.querySelectorAll('code')]
      .filter((code) => !code.closest('pre')).length;
    return {
      role: turn.dataset.turnRole,
      justify: getComputedStyle(turn).justifyContent,
      emerald: bubble.className.includes('emerald'),
      text: content.innerText.replace(/\\s+/g, ' ').trim(),
      strong: content.querySelectorAll('strong').length,
      em: content.querySelectorAll('em').length,
      inlineCode,
      bullets: content.querySelectorAll('ul li').length,
      ordered: content.querySelectorAll('ol li').length,
      quotes: content.querySelectorAll('blockquote').length,
      headings: content.querySelectorAll('p.font-semibold').length,
      code: [...content.querySelectorAll('pre')].map((pre) => ({
        lang: pre.parentElement?.querySelector('span')?.textContent ?? '',
        code: pre.textContent,
      })),
    };
  });
  return {
    title: drawer.querySelector('h2')?.textContent,
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    turns,
  };
})()`;

function assertDrawer(harness, drawer) {
  if (!drawer) throw new Error(`${harness}: drawer did not open`);
  if (drawer.turns.length < 2) throw new Error(`${harness}: no conversation rendered`);
  if (drawer.overflow !== 0)
    throw new Error(`${harness}: horizontal overflow ${drawer.overflow}px`);

  for (const turn of drawer.turns) {
    if (turn.role !== "user" && turn.role !== "assistant") {
      throw new Error(`${harness}: unexpected role ${turn.role}`);
    }
    const expected = turn.role === "user" ? "flex-end" : "flex-start";
    if (turn.justify !== expected) {
      throw new Error(`${harness}: ${turn.role} bubble is ${turn.justify}, expected ${expected}`);
    }
    if (turn.role === "user" && !turn.emerald) {
      throw new Error(`${harness}: user bubble is not emerald`);
    }
    const leak =
      /Created At:|Completed At:|The command exited with code|<rules>|<user_info>|<user_query>|<system-reminder>|\*\*|## /.exec(
        turn.text,
      );
    if (leak) throw new Error(`${harness}: leaked "${leak[0]}" in ${turn.role} turn`);
    if (turn.text.length === 0) throw new Error(`${harness}: empty ${turn.role} bubble`);
  }
}

function firstDifference(actual, expected, path = "") {
  const a = JSON.stringify(actual);
  const b = JSON.stringify(expected);
  if (a === b) return null;
  if (
    actual === null ||
    expected === null ||
    typeof actual !== "object" ||
    typeof expected !== "object"
  ) {
    return `${path || "<root>"}: expected ${b}, got ${a}`;
  }
  for (const key of new Set([...Object.keys(actual), ...Object.keys(expected)])) {
    const nested = firstDifference(actual[key], expected[key], path ? `${path}.${key}` : key);
    if (nested) return nested;
  }
  return `${path || "<root>"}: snapshots differ`;
}

async function main() {
  mkdirSync(TMP, { recursive: true });
  const profile = mkdtempSync(join(TMP, "orquestra-visual-"));
  const shots = join(TMP, "visual-snapshots");
  mkdirSync(shots, { recursive: true });

  const vite = startVite();
  let chrome;
  let page;
  try {
    await waitForServer(vite);
    chrome = await launchChrome({ url: APP_URL, profileDir: profile });
    page = await connectPage(chrome.port);
    await page.send("Page.enable");
    await page.send("Runtime.enable");
    await openSessionsList(page);

    const drawers = [];
    for (const harness of HARNESSES) {
      if (!(await page.evaluate(SNAPSHOT_EXPRESSION(harness)))) {
        throw new Error(`${harness}: fixture row is missing from the gallery`);
      }
      await page.waitFor("!!document.querySelector('[role=dialog]')", `${harness} drawer`);
      await delay(120);
      const drawer = await page.evaluate(READ_DRAWER);
      assertDrawer(harness, drawer);
      drawers.push({ harness, ...drawer });
      await page.screenshot(join(shots, `${harness}.png`));
      await page.evaluate(
        "window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); true",
      );
      await page.waitFor("!document.querySelector('[role=dialog]')", `close ${harness}`);
    }

    const actual = { version: 1, drawers };
    const rendered = `${JSON.stringify(actual, null, 2)}\n`;

    if (process.env.UPDATE_SNAPSHOTS === "1") {
      mkdirSync(dirname(GOLDEN), { recursive: true });
      writeFileSync(GOLDEN, rendered);
      console.log(`visual snapshots updated: ${GOLDEN}`);
      return;
    }

    let expected;
    try {
      expected = JSON.parse(readFileSync(GOLDEN, "utf8"));
    } catch {
      throw new Error(
        `Missing or invalid golden snapshot ${GOLDEN}. Run UPDATE_SNAPSHOTS=1 npm run test:visual.`,
      );
    }

    const difference = firstDifference(actual, expected);
    if (difference) {
      writeFileSync(join(TMP, "visual-actual.json"), rendered);
      throw new Error(
        `Transcript drawer drift:\\n${difference}\\n\\nActual: ${join(TMP, "visual-actual.json")}\\n` +
          `Refresh intentionally with UPDATE_SNAPSHOTS=1 npm run test:visual.`,
      );
    }

    console.log(
      `visual snapshots match: ${drawers.length} drawers, ${drawers.reduce(
        (total, drawer) => total + drawer.turns.length,
        0,
      )} turns`,
    );
  } finally {
    page?.close();
    chrome?.child.kill("SIGKILL");
    vite.child.kill("SIGTERM");
    await delay(200);
    rmSync(profile, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
