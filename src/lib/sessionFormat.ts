/** Shared formatting for the Sessions view. Kept tiny so every sessions component can import it without cycles. */

/** 1024-based human size: "42 KB", "14.5 MB", "2.4 GB". */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
}

/** Short MB string for toasts: "12.4 MB". */
export function formatMb(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const DAY_MS = 86_400_000;

/** Relative age in the active locale, e.g. "há 18 dias" / "18d ago". */
export function relativeAge(iso: string, locale: string): string {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return iso;
  const days = Math.max(0, Math.floor((Date.now() - then) / DAY_MS));
  if (days === 0) return locale.startsWith("pt") ? "hoje" : "today";
  if (days === 1) return locale.startsWith("pt") ? "há 1 dia" : "1d ago";
  return locale.startsWith("pt") ? `há ${days} dias` : `${days}d ago`;
}

/** Explicit local date + time: `18/10/2026 05:41`. Zero-padded and
 * locale-independent so sessions stay identifiable at a glance. */
export function formatDateTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(date.getDate())}/${pad(date.getMonth() + 1)}/${date.getFullYear()} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** Absolute date for tooltips: locale short date + time. */
export function absoluteDate(iso: string, locale: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  try {
    return date.toLocaleString(locale === "pt" ? "pt-BR" : "en-US", {
      dateStyle: "medium",
      timeStyle: "short",
    });
  } catch {
    return iso;
  }
}

/** Shorten a workspace path to its last two segments. */
export function shortPath(path: string | null): string {
  if (!path) return "—";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 2) return path;
  return `…/${parts.slice(-2).join("/")}`;
}

/** Brand color per harness for badges. */
export function harnessColor(harness: string): string {
  switch (harness) {
    case "antigravity":
      return "text-violet-300 border-violet-500/25 bg-violet-500/10";
    case "codex":
      return "text-emerald-300 border-emerald-500/25 bg-emerald-500/10";
    case "claude":
      return "text-orange-300 border-orange-500/25 bg-orange-500/10";
    case "freebuff":
      return "text-cyan-300 border-cyan-500/25 bg-cyan-500/10";
    case "opencode":
      return "text-sky-300 border-sky-500/25 bg-sky-500/10";
    case "kilo":
      return "text-teal-300 border-teal-500/25 bg-teal-500/10";
    case "copilot":
      return "text-blue-300 border-blue-500/25 bg-blue-500/10";
    case "hermes":
      return "text-fuchsia-300 border-fuchsia-500/25 bg-fuchsia-500/10";
    case "grok":
      return "text-zinc-200 border-white/15 bg-white/[0.05]";
    default:
      return "text-zinc-300 border-white/10 bg-white/[0.04]";
  }
}
