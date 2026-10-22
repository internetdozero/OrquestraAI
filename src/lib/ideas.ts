import { invoke } from "@tauri-apps/api/core";
import {
  devMockMode,
  fixtureCreateIdea,
  fixtureProductivity,
  fixtureDeleteIdea,
  fixtureListIdeaProjects,
  fixtureListIdeas,
  fixtureListNotes,
  fixtureMarkNoteRead,
  fixturePostNote,
  fixtureUpdateIdea,
  fixtureUpdateIdeaStatus,
} from "./fixtures.ts";

/** Closed vocabularies, mirroring the Rust core and the Python MCP. */
export type IdeaCategory = "feature" | "architecture" | "bug" | "ux" | "idea";
export type IdeaPriority = "low" | "medium" | "high" | "critical";
export type IdeaStatus = "open" | "in_progress" | "completed" | "archived";

/** Mirrors the Rust `IdeaItem` struct: one row of the `ideas` table. */
export interface IdeaItem {
  id: number;
  project: string;
  title: string;
  description: string | null;
  category: string;
  priority: string;
  /** Tags exactly as stored (usually a JSON array string, as the MCP writes). */
  tags: string | null;
  status: string;
  createdAt: string;
  updatedAt: string;
  /** When the idea last entered `completed`; null unless it is (or was) done. */
  completedAt: string | null;
  /** Exact technical English Builder prompt; null when no task attached. */
  prompt: string | null;
}

/** Mirrors the Rust `BlackboardNote` struct. */
export interface BlackboardNote {
  id: number;
  project: string;
  topic: string;
  sender: string;
  target: string;
  note: string;
  isRead: boolean;
  createdAt: string;
}

/** Mirrors the Rust `CreateIdeaInput`; omitted enums take MCP defaults. */
export interface CreateIdeaInput {
  project: string;
  title: string;
  description?: string | null;
  category?: string | null;
  priority?: string | null;
  tags?: string | null;
  prompt?: string | null;
}

/** Mirrors the Rust `UpdateIdeaInput`; only set fields are written. */
export interface UpdateIdeaInput {
  id: number;
  project?: string | null;
  title?: string | null;
  description?: string | null;
  category?: string | null;
  priority?: string | null;
  tags?: string | null;
  status?: string | null;
  prompt?: string | null;
}

/** Mirrors the Rust `PostNoteInput`; `target` defaults to `"all"`. */
export interface PostNoteInput {
  project: string;
  topic: string;
  sender: string;
  target?: string | null;
  note: string;
}

/**
 * Ideas, highest priority first. A missing status defaults to `"open"`;
 * pass `"all"` to disable the status filter. `limit`/`offset` page the
 * result; omitting them returns the backend's default first page (500).
 */
export function listIdeas(
  project?: string | null,
  status?: string | null,
  limit?: number | null,
  offset?: number | null,
): Promise<IdeaItem[]> {
  if (devMockMode()) return Promise.resolve(fixtureListIdeas(project, status));
  return invoke<IdeaItem[]>("list_ideas", {
    project: project ?? null,
    status: status ?? null,
    limit: limit ?? null,
    offset: offset ?? null,
  });
}

/** Distinct project names for the filter dropdown. */
export function listIdeaProjects(): Promise<string[]> {
  if (devMockMode()) return Promise.resolve(fixtureListIdeaProjects());
  return invoke<string[]>("list_idea_projects");
}

/** Record one idea; resolves to the stored row. */
export function createIdea(input: CreateIdeaInput): Promise<IdeaItem> {
  if (devMockMode()) return Promise.resolve(fixtureCreateIdea(input));
  return invoke<IdeaItem>("create_idea", { input });
}

/** Partial update; resolves to the stored row. */
export function updateIdea(input: UpdateIdeaInput): Promise<IdeaItem> {
  if (devMockMode()) return Promise.resolve(fixtureUpdateIdea(input));
  return invoke<IdeaItem>("update_idea", { input });
}

/** Move one idea to a new status; resolves to the stored row. */
export function updateIdeaStatus(id: number, status: string): Promise<IdeaItem> {
  if (devMockMode()) return Promise.resolve(fixtureUpdateIdeaStatus(id, status));
  return invoke<IdeaItem>("update_idea_status", { id, status });
}

/** Delete one idea; rejects on an unknown id. */
export function deleteIdea(id: number): Promise<void> {
  if (devMockMode()) return Promise.resolve(fixtureDeleteIdea(id));
  return invoke<void>("delete_idea", { id });
}

/**
 * Blackboard notes oldest first, optionally unread only and/or one project.
 * Paged like ideas; omitting `limit` returns the default page.
 */
export function listBlackboardNotes(
  project?: string | null,
  unreadOnly?: boolean,
  limit?: number | null,
): Promise<BlackboardNote[]> {
  if (devMockMode()) return Promise.resolve(fixtureListNotes(project, unreadOnly));
  return invoke<BlackboardNote[]>("list_blackboard_notes", {
    project: project ?? null,
    unreadOnly: unreadOnly ?? true,
    limit: limit ?? null,
    offset: null,
  });
}

/** Post a note for other agents; resolves to the stored row. */
export function postBlackboardNote(input: PostNoteInput): Promise<BlackboardNote> {
  if (devMockMode()) return Promise.resolve(fixturePostNote(input));
  return invoke<BlackboardNote>("post_blackboard_note", { input });
}

/** Mark one note read; rejects on an unknown id. */
export function markNoteRead(id: number): Promise<void> {
  if (devMockMode()) return Promise.resolve(fixtureMarkNoteRead(id));
  return invoke<void>("mark_note_read", { id });
}

/** One heatmap cell: Idea Vault deliveries for a calendar day. */
export interface DayActivity {
  date: string;
  ideasCompleted: number;
  intensity: 0 | 1 | 2 | 3 | 4;
}

/** Ideas completed in the trailing 7 days versus the 7 before them. */
export interface WeeklyVelocity {
  thisWeek: number;
  lastWeek: number;
}

/** Everything the productivity sub-panel renders in one round trip. */
export interface ProductivityMetrics {
  dailyActivity: DayActivity[];
  currentStreak: number;
  weeklyVelocity: WeeklyVelocity;
  avgCycleHours: number | null;
  project: string | null;
}

/**
 * Developer velocity: Idea Vault completions per day. `weeks` defaults to
 * 16 on the core side; `project` scopes the deliveries.
 */
export function getProductivityMetrics(
  project?: string | null,
  weeks?: number | null,
): Promise<ProductivityMetrics> {
  if (devMockMode()) return Promise.resolve(fixtureProductivity(project, weeks));
  return invoke<ProductivityMetrics>("get_productivity_metrics", {
    project: project ?? null,
    weeks: weeks ?? null,
  });
}

/**
 * Format an idea as a markdown-structured prompt ready for AI agent dispatch.
 */
export function formatIdeaForAi(idea: IdeaItem): string {
  const lines = [
    `Task #${idea.id}: ${idea.title}`,
    `Project: ${idea.project}`,
    `Category: ${idea.category} | Priority: ${idea.priority} | Status: ${idea.status}`,
  ];
  if (idea.tags && idea.tags.trim()) {
    lines.push(`Tags: ${idea.tags}`);
  }
  lines.push("\nDescription:");
  lines.push(idea.description || "(No description provided)");
  if (idea.prompt && idea.prompt.trim()) {
    lines.push("\nBuilder Prompt:");
    lines.push(idea.prompt);
  }
  return lines.join("\n");
}

/** Notion-style sortable columns of the ideas list header. */
export type IdeaSortField = "title" | "project" | "category" | "priority" | "updated";

const PRIORITY_RANK: Record<string, number> = {
  low: 0,
  medium: 1,
  high: 2,
  critical: 3,
};

/** Row comparator for one list column. Unknown priorities sort below `low`. */
export function compareIdeas(field: IdeaSortField, a: IdeaItem, b: IdeaItem): number {
  switch (field) {
    case "title":
      return a.title.localeCompare(b.title);
    case "project":
      return a.project.localeCompare(b.project);
    case "category":
      return a.category.localeCompare(b.category);
    case "priority":
      return (PRIORITY_RANK[a.priority] ?? -1) - (PRIORITY_RANK[b.priority] ?? -1);
    case "updated":
      return (
        Date.parse(a.updatedAt.replace(" ", "T") + "Z") -
        Date.parse(b.updatedAt.replace(" ", "T") + "Z")
      );
  }
}

/**
 * Copy the AI prompt for one idea. Resolves `true` on success, `false` when
 * the clipboard denies permission — never throws.
 */
export async function copyIdeaForAi(idea: IdeaItem): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(formatIdeaForAi(idea));
    return true;
  } catch {
    return false;
  }
}
