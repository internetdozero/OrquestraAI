/**
 * Minimal Chrome DevTools Protocol client for the visual snapshot runner.
 *
 * No browser package is added: the runner drives the local Chrome binary that
 * the desktop app already targets, over the same CDP surface used by devtools.
 * Chrome is launched on an ephemeral port and the port is read back from its
 * profile, so concurrent runs never collide.
 */
import { spawn, spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const CANDIDATES =
  process.platform === "darwin"
    ? ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"]
    : process.platform === "win32"
      ? [
          "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
          "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
        ]
      : ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"];

/** First Chrome/Chromium on PATH, or an explicit `CHROME_PATH` override. */
export function findChrome() {
  const candidates = process.env.CHROME_PATH
    ? [process.env.CHROME_PATH, ...CANDIDATES]
    : CANDIDATES;

  for (const candidate of candidates) {
    if (candidate.includes("/") || candidate.includes("\\")) {
      if (existsSync(candidate)) return candidate;
      continue;
    }
    const probe = spawnSync(candidate, ["--version"], { stdio: "ignore" });
    if (probe.status === 0) return candidate;
  }
  return null;
}

/** Launch headless Chrome at `url`; resolves once the CDP port is readable. */
export async function launchChrome({ url, profileDir, width = 1100, height = 900 }) {
  const binary = findChrome();
  if (!binary) {
    throw new Error(
      "Chrome/Chromium not found. Install Chrome or set CHROME_PATH to run the visual snapshot gate.",
    );
  }

  const child = spawn(
    binary,
    [
      "--headless=new",
      "--no-sandbox",
      "--disable-gpu",
      "--hide-scrollbars",
      "--no-first-run",
      "--no-default-browser-check",
      "--remote-debugging-address=127.0.0.1",
      "--remote-debugging-port=0",
      `--user-data-dir=${profileDir}`,
      `--window-size=${width},${height}`,
      url,
    ],
    { stdio: ["ignore", "ignore", "pipe"] },
  );

  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  const portFile = join(profileDir, "DevToolsActivePort");
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`Chrome exited (${child.exitCode}): ${stderr.slice(-2000)}`);
    }
    if (existsSync(portFile)) {
      const port = readFileSync(portFile, "utf8").split("\n")[0].trim();
      if (port) return { child, port: Number(port), stderr: () => stderr };
    }
    await delay(100);
  }

  child.kill("SIGKILL");
  throw new Error(`Chrome did not open a CDP port: ${stderr.slice(-2000)}`);
}

/** Attach to the first page target exposed by Chrome. */
export async function connectPage(port) {
  const deadline = Date.now() + 15_000;
  let page;
  while (Date.now() < deadline) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      page = list.find((target) => target.type === "page" && target.webSocketDebuggerUrl);
      if (page) break;
    } catch {
      /* Chrome is still starting. */
    }
    await delay(100);
  }
  if (!page) throw new Error("Chrome exposed no page target");

  const ws = new WebSocket(page.webSocketDebuggerUrl);
  const pending = new Map();
  let nextId = 0;
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    const resolver = message.id !== undefined ? pending.get(message.id) : undefined;
    if (typeof resolver === "function") {
      pending.delete(message.id);
      resolver(message);
    }
  };
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = () => reject(new Error("CDP WebSocket failed"));
  });

  const send = (method, params = {}) =>
    new Promise((resolve) => {
      const id = ++nextId;
      pending.set(id, resolve);
      ws.send(JSON.stringify({ id, method, params }));
    });

  const evaluate = async (expression) => {
    const reply = await send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
    });
    if (reply.result?.exceptionDetails) {
      throw new Error(JSON.stringify(reply.result.exceptionDetails));
    }
    return reply.result?.result?.value;
  };

  const waitFor = async (expression, label, timeout = 15_000) => {
    const until = Date.now() + timeout;
    while (Date.now() < until) {
      if (await evaluate(expression)) return;
      await delay(100);
    }
    throw new Error(`Timed out waiting for ${label}`);
  };

  const screenshot = async (path) => {
    const shot = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(path, Buffer.from(shot.result.data, "base64"));
  };

  return { send, evaluate, waitFor, screenshot, close: () => ws.close() };
}
