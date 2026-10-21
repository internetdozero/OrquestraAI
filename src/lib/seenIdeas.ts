import type { IdeaItem } from "./ideas.ts";

/**
 * Persistence for the [NEW] chip on freshly created vault tasks.
 *
 * An idea is new while it was created inside the freshness window and the
 * operator has not opened it yet. Opening it is the only dismissal: the row
 * or card click records the id here, so the chip disappears for good instead
 * of returning on the next poll.
 */

export const SEEN_IDEAS_KEY = "orquestra:seen-ideas";

/** How long an untouched idea keeps its [NEW] chip. */
export const NEW_IDEA_WINDOW_MS = 10 * 60 * 1000;

/** Upper bound so the stored list cannot grow with the whole vault. */
const MAX_SEEN_IDS = 500;

/** Parse the vault's `YYYY-MM-DD HH:MM:SS` UTC stamps (NaN when malformed). */
export function ideaCreatedMs(createdAt: string): number {
  return Date.parse(createdAt.replace(" ", "T") + "Z");
}

/** Seen ids, oldest first. Denied or corrupt storage reads as empty. */
export function readSeenIdeaIds(): number[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(SEEN_IDEAS_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (entry): entry is number => typeof entry === "number" && Number.isInteger(entry),
    );
  } catch {
    return [];
  }
}

/** Persist seen ids, keeping only the newest ones. */
export function writeSeenIdeaIds(ids: Iterable<number>): void {
  if (typeof window === "undefined") return;
  try {
    const kept = [...new Set(ids)].slice(-MAX_SEEN_IDS);
    window.localStorage.setItem(SEEN_IDEAS_KEY, JSON.stringify(kept));
  } catch {
    // Persisting the marker is a nicety; a denied store keeps memory state.
  }
}

/** Ids that should carry the [NEW] chip right now. */
export function collectNewIdeaIds(
  ideas: Iterable<Pick<IdeaItem, "id" | "createdAt">>,
  seen: ReadonlySet<number>,
  now: number = Date.now(),
): Set<number> {
  const ids = new Set<number>();
  for (const idea of ideas) {
    if (isNewIdea(idea, seen, now)) ids.add(idea.id);
  }
  return ids;
}

/** True while an unopened idea is still inside the freshness window. */
export function isNewIdea(
  idea: Pick<IdeaItem, "id" | "createdAt">,
  seen: ReadonlySet<number>,
  now: number = Date.now(),
): boolean {
  if (seen.has(idea.id)) return false;
  const created = ideaCreatedMs(idea.createdAt);
  if (Number.isNaN(created)) return false;
  const age = now - created;
  return age >= 0 && age < NEW_IDEA_WINDOW_MS;
}
