import { FIXTURE_IDEAS, FIXTURE_NOTES } from "../mocks/ideaFixtures.ts";
import {
  FIXTURE_MCP_SERVERS,
  FIXTURE_MCP_TARGETS,
  FIXTURE_OFFICIAL_MCPS,
} from "../mocks/mcpFixtures.ts";
import { FIXTURE_SESSIONS, fixtureTranscript } from "../mocks/transcriptFixtures.ts";
import type {
  BlackboardNote,
  CreateIdeaInput,
  IdeaItem,
  PostNoteInput,
  ProductivityMetrics,
  UpdateIdeaInput,
} from "./ideas";
import type { McpServer, McpTarget } from "./mcps";
import type {
  PurgeEstimate,
  PurgeFilter,
  PurgeReport,
  RetentionPolicy,
  TranscriptTurn,
  UnifiedSession,
} from "./sessions";
import type { OfficialMcpEntry, OfficialStore } from "./tauri";

/**
 * Developer fixture mode: makes the Sessions view and the transcript drawer
 * work in a plain Vite browser preview, with one mock conversation per
 * harness. Never active in a production build.
 *
 * Enable with `?fixtures=1` (remembered in localStorage) and disable with
 * `?fixtures=0`.
 */
const STORAGE_KEY = "orquestra.fixtureMode";

export function fixturesEnabled(): boolean {
  if (!import.meta.env.DEV || typeof window === "undefined") return false;
  // Never shadow real session data inside the desktop app, even in dev.
  if ("__TAURI_INTERNALS__" in window) return false;
  const flag = new URLSearchParams(window.location.search).get("fixtures");
  try {
    if (flag === "1") {
      window.localStorage.setItem(STORAGE_KEY, "1");
      return true;
    }
    if (flag === "0") {
      window.localStorage.removeItem(STORAGE_KEY);
      return false;
    }
    return window.localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return flag === "1";
  }
}

/**
 * Browser mock mode: automatic in Vite dev outside the desktop shell, so
 * agents and developers can click through every view in a plain browser.
 * Unlike `fixturesEnabled` (opt-in per view via `?fixtures=1`), this needs
 * no flag. Never true in production builds or inside Tauri, so mocks can
 * never shadow real data.
 */
export function devMockMode(): boolean {
  if (!import.meta.env.DEV || typeof window === "undefined") return false;
  return !("__TAURI_INTERNALS__" in window);
}

/** Core-shaped timestamp for rows created through the mock mutations. */
function nowStamp(): string {
  return new Date().toISOString().replace("T", " ").slice(0, 19);
}

/** Mutable copies so pin/delete/purge exercise the real UI paths. */
const sessions: UnifiedSession[] = FIXTURE_SESSIONS.map((session) => ({ ...session }));

/** Mutable idea vault / board / catalog so mock edits feel real. */
const ideas: IdeaItem[] = FIXTURE_IDEAS.map((idea) => ({ ...idea }));
const notes: BlackboardNote[] = FIXTURE_NOTES.map((note) => ({ ...note }));
const mcpServers: McpServer[] = FIXTURE_MCP_SERVERS.map((server) => ({
  ...server,
  args: [...server.args],
  env: { ...server.env },
  enabledHarnesses: [...server.enabledHarnesses],
}));

const DEFAULT_RETENTION: RetentionPolicy = {
  defaultTtlDays: 15,
  harnessOverrides: {},
  autoPurgeEnabled: false,
  autoPurgeIntervalHours: 24,
  lastRun: null,
};

let retention: RetentionPolicy = { ...DEFAULT_RETENTION };

export function fixtureSessions(): UnifiedSession[] {
  return sessions.map((session) => ({ ...session }));
}

export function fixtureSessionTranscript(
  harness: string,
  sessionId: string,
): TranscriptTurn[] | undefined {
  return fixtureTranscript(harness, sessionId);
}

export function fixtureRetention(): RetentionPolicy {
  return { ...retention, harnessOverrides: { ...retention.harnessOverrides } };
}

export function fixtureSaveRetention(policy: RetentionPolicy): void {
  retention = { ...policy, harnessOverrides: { ...policy.harnessOverrides } };
}

/** Flip one fixture's pin and report the state after the toggle. */
export function fixtureTogglePin(harness: string, sessionId: string): boolean {
  const session = sessions.find(
    (entry) => entry.harness === harness && entry.id === sessionId,
  );
  if (!session) return false;
  session.isPinned = !session.isPinned;
  if (!session.isPinned) session.pinLabel = null;
  return session.isPinned;
}

function matcher(filter: PurgeFilter): (session: UnifiedSession) => boolean {
  return (session) => {
    if (filter.harness && session.harness !== filter.harness) return false;
    if (filter.sessionIds && !filter.sessionIds.includes(session.id)) return false;
    if (filter.olderThanDays != null) {
      const then = Date.parse(session.updatedAt);
      if (Number.isNaN(then)) return false;
      const age = Date.now() - then;
      if (age < filter.olderThanDays * 86_400_000) return false;
    }
    return true;
  };
}

export function fixtureEstimate(filter: PurgeFilter): PurgeEstimate {
  const matched = sessions.filter(matcher(filter));
  const ignorePinned = filter.ignorePinned ?? true;
  const targets = matched.filter((session) => !(ignorePinned && session.isPinned));
  const harnesses = [...new Set(targets.map((session) => session.harness))].sort();

  return {
    sessionsCount: targets.length,
    reclaimedBytes: targets.reduce((total, session) => total + session.sizeBytes, 0),
    affectedHarnesses: harnesses,
    skippedPinnedCount: matched.length - targets.length,
    skippedActiveCount: 0,
  };
}

/* ------------------------------------------------------------------ */
/* Ideas mock resolvers (mirror lib/ideas filter semantics)              */
/* ------------------------------------------------------------------ */

export function fixtureListIdeas(
  project?: string | null,
  status?: string | null,
): IdeaItem[] {
  return ideas
    .filter((idea) => {
      if (project && idea.project !== project) return false;
      // A missing status defaults to `open`; `all` disables the filter.
      if (status == null) return idea.status === "open";
      if (status === "all") return true;
      return idea.status === status;
    })
    .map((idea) => ({ ...idea }));
}

export function fixtureListIdeaProjects(): string[] {
  return [...new Set(ideas.map((idea) => idea.project))].sort((a, b) =>
    a.localeCompare(b),
  );
}

export function fixtureCreateIdea(input: CreateIdeaInput): IdeaItem {
  const next: IdeaItem = {
    id: ideas.reduce((max, idea) => Math.max(max, idea.id), 0) + 1,
    project: input.project,
    title: input.title,
    description: input.description ?? null,
    category: input.category ?? "idea",
    priority: input.priority ?? "medium",
    tags: input.tags ?? null,
    status: "open",
    createdAt: nowStamp(),
    updatedAt: nowStamp(),
    completedAt: null,
  };
  ideas.push(next);
  return { ...next };
}

export function fixtureUpdateIdea(input: UpdateIdeaInput): IdeaItem {
  const idea = ideas.find((entry) => entry.id === input.id);
  if (!idea) throw new Error(`Unknown idea id ${input.id}`);
  if (input.project != null) idea.project = input.project;
  if (input.title != null) idea.title = input.title;
  if (input.description !== undefined) idea.description = input.description;
  if (input.category != null) idea.category = input.category;
  if (input.priority != null) idea.priority = input.priority;
  if (input.tags !== undefined) idea.tags = input.tags;
  if (input.status != null) {
    idea.status = input.status;
    // Mirror the core: completing stamps, reopening clears, archiving keeps.
    if (input.status === "completed") idea.completedAt = nowStamp();
    else if (input.status === "open" || input.status === "in_progress") {
      idea.completedAt = null;
    }
  }
  idea.updatedAt = nowStamp();
  return { ...idea };
}

export function fixtureUpdateIdeaStatus(id: number, status: string): IdeaItem {
  return fixtureUpdateIdea({ id, status });
}

export function fixtureDeleteIdea(id: number): void {
  const index = ideas.findIndex((entry) => entry.id === id);
  if (index >= 0) ideas.splice(index, 1);
}

export function fixtureListNotes(
  project?: string | null,
  unreadOnly?: boolean,
): BlackboardNote[] {
  return notes
    .filter((note) => {
      if (project && note.project !== project) return false;
      if (unreadOnly && note.isRead) return false;
      return true;
    })
    .map((note) => ({ ...note }));
}

export function fixturePostNote(input: PostNoteInput): BlackboardNote {
  const next: BlackboardNote = {
    id: notes.reduce((max, note) => Math.max(max, note.id), 0) + 1,
    project: input.project,
    topic: input.topic,
    sender: input.sender,
    target: input.target ?? "all",
    note: input.note,
    isRead: false,
    createdAt: nowStamp(),
  };
  notes.push(next);
  return { ...next };
}

export function fixtureMarkNoteRead(id: number): void {
  const note = notes.find((entry) => entry.id === id);
  if (note) note.isRead = true;
}

/* ------------------------------------------------------------------ */
/* MCP mock resolvers (mirror lib/mcps scope semantics)                  */
/* ------------------------------------------------------------------ */

export function fixtureListMcpServers(): McpServer[] {
  return mcpServers.map((server) => ({
    ...server,
    args: [...server.args],
    env: { ...server.env },
    enabledHarnesses: [...server.enabledHarnesses],
  }));
}

export function fixtureListMcpTargets(): McpTarget[] {
  return FIXTURE_MCP_TARGETS.map((target) => ({ ...target }));
}

/** `harnessId` null edits the universal baseline, mirroring the core. */
export function fixtureSetMcpState(
  id: string,
  harnessId: string | null,
  enabled: boolean,
): void {
  const server = mcpServers.find((entry) => entry.id === id);
  if (!server) return;
  if (harnessId === null || harnessId === "global") {
    server.globalEnabled = enabled;
    return;
  }
  server.enabledHarnesses = enabled
    ? [...new Set([...server.enabledHarnesses, harnessId])]
    : server.enabledHarnesses.filter((entry) => entry !== harnessId);
}

export function fixtureBatchSetMcpAll(
  harnessId: string | null,
  enabled: boolean,
): void {
  for (const server of mcpServers) {
    if (harnessId === null || harnessId === "global") {
      server.globalEnabled = enabled;
    } else {
      server.enabledHarnesses = enabled
        ? [...new Set([...server.enabledHarnesses, harnessId])]
        : server.enabledHarnesses.filter((entry) => entry !== harnessId);
    }
  }
}

export function fixtureSaveMcp(server: McpServer): void {
  const index = mcpServers.findIndex((entry) => entry.id === server.id);
  const copy: McpServer = {
    ...server,
    args: [...server.args],
    env: { ...server.env },
    enabledHarnesses: [...server.enabledHarnesses],
  };
  if (index >= 0) mcpServers[index] = copy;
  else mcpServers.push(copy);
}

export function fixtureDeleteMcp(id: string): void {
  const index = mcpServers.findIndex((entry) => entry.id === id);
  if (index >= 0) mcpServers.splice(index, 1);
}

export function fixtureRevealMcpSecret(id: string, key: string): string {
  const server = mcpServers.find((entry) => entry.id === id);
  if (!server) return "";
  return server.env[key] ?? "";
}

export function fixtureDeployMcps(): string[] {
  return [];
}

/** Deterministic demo velocity: seeded per project so the heatmap paints a
 *  stable, GitHub-plausible wave anchored to today. Same streak / velocity /
 *  intensity rules as the Rust core; cycles average 4-72h per delivery. */
export function fixtureProductivity(
  project?: string | null,
  weeks?: number | null,
): ProductivityMetrics {
  const totalWeeks = Math.min(Math.max(weeks ?? 16, 1), 52);
  const days = totalWeeks * 7;
  let seed = 0x9e37;
  const key = project ?? "all";
  for (let i = 0; i < key.length; i += 1) seed = (seed * 31 + key.charCodeAt(i)) >>> 0;
  const random = () => {
    seed = (seed + 0x6d2b79f5) >>> 0;
    let t = seed;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const pad = (n: number) => String(n).padStart(2, "0");
  const today = new Date();
  const dailyActivity = Array.from({ length: days }, (_, index) => {
    const date = new Date(today);
    date.setDate(today.getDate() - (days - 1 - index));
    const weekend = date.getDay() === 0 || date.getDay() === 6;
    const ideasCompleted = random() < (weekend ? 0.08 : 0.22) ? 1 + Math.floor(random() * 2) : 0;
    return {
      date: `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`,
      ideasCompleted,
      intensity: (ideasCompleted === 0 ? 0 : ideasCompleted === 1 ? 1 : ideasCompleted === 2 ? 2 : ideasCompleted === 3 ? 3 : 4) as 0 | 1 | 2 | 3 | 4,
    };
  });
  let cursor = dailyActivity.length;
  const last = dailyActivity[cursor - 1];
  if (last.ideasCompleted === 0) cursor -= 1;
  let currentStreak = 0;
  while (cursor > 0) {
    const day = dailyActivity[cursor - 1];
    if (day.ideasCompleted === 0) break;
    currentStreak += 1;
    cursor -= 1;
  }
  const week = dailyActivity.slice(-7).reduce((sum, day) => sum + day.ideasCompleted, 0);
  const previous = dailyActivity
    .slice(-14, -7)
    .reduce((sum, day) => sum + day.ideasCompleted, 0);
  const deliveries = dailyActivity.reduce((sum, day) => sum + day.ideasCompleted, 0);
  return {
    dailyActivity,
    currentStreak,
    weeklyVelocity: { thisWeek: week, lastWeek: previous },
    avgCycleHours: deliveries === 0 ? null : Math.round((18 + random() * 30) * 10) / 10,
    project: project ?? null,
  };
}

export function fixtureOfficialStore(): OfficialStore {
  return {
    skills: [],
    mcps: FIXTURE_OFFICIAL_MCPS.map((entry) => ({
      ...entry,
      args: [...entry.args],
    })) as OfficialMcpEntry[],
  };
}

export function fixturePurge(filter: PurgeFilter): PurgeReport {
  const ignorePinned = filter.ignorePinned ?? true;
  const matches = matcher(filter);
  let deletedCount = 0;
  let reclaimedBytes = 0;

  for (let index = sessions.length - 1; index >= 0; index -= 1) {
    const session = sessions[index];
    if (!matches(session) || (ignorePinned && session.isPinned)) continue;
    deletedCount += 1;
    reclaimedBytes += session.sizeBytes;
    sessions.splice(index, 1);
  }

  return { deletedCount, reclaimedBytes, failedCount: 0, errors: [] };
}
