import type { IdeaItem } from "./ideas";

interface CachedIdeasData {
  ideas: IdeaItem[];
  projects: string[];
  timestamp: number;
}

let memoryCache: CachedIdeasData | null = null;

export function readIdeasCache(): CachedIdeasData | null {
  return memoryCache;
}

export function writeIdeasCache(ideas: IdeaItem[], projects: string[]): void {
  memoryCache = {
    ideas,
    projects,
    timestamp: Date.now(),
  };
}

export function dropIdeasCache(): void {
  memoryCache = null;
}
