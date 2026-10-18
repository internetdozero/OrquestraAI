import type { TranslationSchema } from "../pt";

/** English mirror of `pt/janitor.ts`. A missing or extra key fails the build. */
export const janitor: TranslationSchema["janitor"] = {
  title: "System health",
  subtitle: "Live metrics and safe temporary-file cleanup.",
  refresh: "Refresh metrics",
  alert: "Memory or /tmp usage is above the recommended threshold.",
  ram: "RAM",
  swap: "Swap",
  tmp: "/tmp",
  cpu: "CPU load",
  cores: "cores",
  mb: "MB",
  topProcesses: "Top processes by RAM",
  tmpPath: (path: string, isTmpfs: boolean) => `${path}${isTmpfs ? " · tmpfs" : ""}`,
  noProcesses: "No visible processes.",
  loading: "Reading system metrics…",
  unavailable: "Metrics unavailable.",
  sweepHint: "Only inactive sockets and scratch files older than 6 hours are removed.",
  sweep: "Sweep stale caches",
  sweeping: "Sweeping…",
  sweepComplete: (count: number, mb: number) =>
    `${count} file(s) removed · ${mb.toFixed(1)} MB reclaimed`,
  ramAlert: (percent: number, threshold: number) =>
    `RAM at ${percent}% — above the ${threshold}% threshold`,
  sweepFailed: (detail: string) => `Cache sweep failed: ${detail}`,
  readFailed: (detail: string) => `Could not read system metrics: ${detail}`,
};
