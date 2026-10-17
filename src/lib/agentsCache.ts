import { readMasterAgentsMd } from "./tauri";

let text: string | null = null;
let inflight: Promise<string> | null = null;

/** Master instructions already read this session, or `null` if still unknown. */
export function readAgentsCache(): string | null {
  return text;
}

/** Remember a saved draft so remounting the editor does not reload it. */
export function writeAgentsCache(next: string): void {
  text = next;
}

/** One read of `AGENTS.md`, shared with the background preload. */
export function ensureAgentsText(): Promise<string> {
  if (text !== null) return Promise.resolve(text);
  if (inflight) return inflight;

  inflight = readMasterAgentsMd()
    .then((value) => {
      text = value;
      return value;
    })
    .finally(() => {
      inflight = null;
    });

  return inflight;
}
