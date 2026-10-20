import {
  FIXTURE_MCP_SERVERS,
  FIXTURE_MCP_TARGETS,
  FIXTURE_OFFICIAL_MCPS,
} from "../../mocks/mcpFixtures.ts";
import type { ProductivityMetrics } from "../ideas";
import type { McpServer, McpTarget } from "../mcps";
import type { OfficialMcpEntry, OfficialStore } from "../tauri";
import type { SkillItem } from "../skills";

const mcpServers: McpServer[] = FIXTURE_MCP_SERVERS.map((server) => ({
  ...server,
  args: [...server.args],
  env: { ...server.env },
  enabledHarnesses: [...server.enabledHarnesses],
}));

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

export function fixtureSkills(count = 420): SkillItem[] {
  const categories = ["Security", "Database", "Cloud", "Infra", "Frontend", "AI & Agents"];
  return Array.from({ length: count }, (_, i) => {
    const isGlobal = i < 2;
    const cat = categories[i % categories.length];
    return {
      id: `skill-${i}`,
      name: `skill-${String(i).padStart(3, "0")}-${cat.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`,
      description:
        i % 3 === 0
          ? "Deep structural analysis and refactoring workflows across multiple repositories with strict type boundaries and runtime verifications."
          : i % 3 === 1
            ? "Short single-line description."
            : "Medium description spanning roughly two lines of text in the three-column card grid layout for realistic vertical variance across cards.",
      category: cat,
      official: i % 7 === 0,
      tokens: 800 + (i % 20) * 150,
      globalEnabled: isGlobal,
      enabledHarnesses: isGlobal ? ["antigravity", "claude_code"] : [],
      harnessEnabled: isGlobal,
      inheritedFromGlobal: false,
    };
  });
}

