/**
 * Opt-in timing for the two moments the cache work was meant to improve: boot
 * and the switch to another view.
 *
 * Timing the IPC commands alone stopped being meaningful once the views served
 * cache: the interesting number is how long the operator waits between the
 * click and the first real data on screen, and whether that data came from
 * memory or from the core. These marks answer both, per view.
 *
 * Everything is inert until enabled, which keeps a normal session free of
 * `performance` entries and globals:
 *
 * - dev builds: on by default;
 * - release builds: `__ownhubPerf.enable()` in the webview console, or
 *   `localStorage.setItem("ownhub:perf", "1")` before launching;
 * - read the numbers with `__ownhubPerf.report()` (copy as JSON) and clear
 *   with `__ownhubPerf.clear()`.
 */

export type PerfSource = "cache" | "load";

export interface PerfEntry {
  /** `boot:<phase>` or `view:<id>`. */
  name: string;
  /** Milliseconds since the matching start. */
  ms: number;
  /** For views: whether the first paint came from cache or from the core. */
  source?: PerfSource;
  /** Wall clock of the measurement, for correlating with logs. */
  at: string;
}

const STORAGE_KEY = "ownhub:perf";
const MAX_ENTRIES = 500;

/** Bundle evaluation is the earliest instant this app can observe. */
const bootAt = now();
const entries: PerfEntry[] = [];
let enabled: boolean | null = null;

function now(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

function devBuild(): boolean {
  try {
    return Boolean(import.meta.env.DEV);
  } catch {
    return false; // plain Node, where `import.meta.env` does not exist
  }
}

function storedFlag(): boolean {
  try {
    // Browser storage, not the Node stub: reading the global there warns.
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

/** Whether marks are being collected right now. */
export function perfEnabled(): boolean {
  if (enabled === null) enabled = devBuild() || storedFlag();
  return enabled;
}

/** Turn collection on or off and remember it for the next launch. */
export function setPerfEnabled(on: boolean): void {
  enabled = on;
  try {
    if (typeof window !== "undefined") {
      if (on) window.localStorage.setItem(STORAGE_KEY, "1");
      else window.localStorage.removeItem(STORAGE_KEY);
    }
  } catch {
    /* Private mode or a locked-down webview: the session-only flag still holds. */
  }
}

function record(name: string, ms: number, source?: PerfSource): PerfEntry {
  const entry: PerfEntry = {
    name,
    ms: Math.round(ms * 10) / 10,
    source,
    at: new Date().toISOString(),
  };
  entries.push(entry);
  if (entries.length > MAX_ENTRIES) entries.splice(0, entries.length - MAX_ENTRIES);
  return entry;
}

/**
 * Boot phase, measured from bundle evaluation: `shell` is the first frame the
 * operator can interact with, `preload-done` the end of the idle warm-up.
 */
export function perfBoot(phase: string): void {
  if (!perfEnabled()) return;
  record(`boot:${phase}`, now() - bootAt);
  try {
    // Anchored at the time origin, so DevTools shows it on the same axis as the
    // navigation and the first paint.
    performance.measure(`ownhub:boot:${phase}`, { start: 0, duration: now() - bootAt });
  } catch {
    /* A missing Performance API must never break the shell. */
  }
}

let pendingSwitch: { view: string; start: number; reported: boolean } | null = null;

/** Call on the click/tab switch, before the next view renders. */
export function perfViewSwitch(view: string): void {
  if (!perfEnabled()) return;
  pendingSwitch = { view, start: now(), reported: false };
  try {
    performance.mark(`ownhub:view:${view}:start`);
  } catch {
    /* Optional: the recorded entry does not depend on it. */
  }
}

/**
 * Call when the view first shows data. `source` separates a cache paint from a
 * core read, which is the whole point: the first should be a few ms, the
 * second carries the IPC cost. Later revalidations are ignored so one switch
 * produces exactly one measurement.
 */
export function perfViewPainted(view: string, source: PerfSource): void {
  if (!perfEnabled() || !pendingSwitch || pendingSwitch.reported) return;
  if (pendingSwitch.view !== view) return;

  pendingSwitch.reported = true;
  const ms = now() - pendingSwitch.start;
  record(`view:${view}`, ms, source);
  try {
    performance.measure(`ownhub:view:${view}:${source}`, {
      start: `ownhub:view:${view}:start`,
      duration: ms,
    });
    performance.clearMarks(`ownhub:view:${view}:start`);
  } catch {
    /* DevTools spans are a bonus, the entry above is the record. */
  }
}

/** Every measurement collected so far, oldest first. */
export function perfReport(): PerfEntry[] {
  return [...entries];
}

/** Drop collected measurements, keeping the enabled flag. */
export function perfClear(): void {
  entries.length = 0;
  pendingSwitch = null;
}

/** Expose the console handle used to enable and read the timings in a build. */
export function installPerfConsole(): void {
  if (typeof window === "undefined") return;

  (window as Window & { __ownhubPerf?: unknown }).__ownhubPerf = {
    enabled: perfEnabled,
    enable: () => setPerfEnabled(true),
    disable: () => setPerfEnabled(false),
    report: perfReport,
    clear: perfClear,
  };
}
