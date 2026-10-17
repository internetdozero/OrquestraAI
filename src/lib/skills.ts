import type { LucideIcon } from "lucide-react";
import {
  Bot,
  Boxes,
  CalendarDays,
  Cloud,
  Database,
  FileText,
  FlaskConical,
  Image,
  LayoutGrid,
  MessageSquare,
  Server,
  ShieldCheck,
  Wrench,
} from "lucide-react";

/** Mirrors the `SkillItem` struct returned by the Rust `list_skills` command. */
export interface SkillItem {
  id: string;
  name: string;
  description: string;
  category: string;
  /** Approximate prompt weight of the skill definition. */
  tokens: number;
  /** Projected in the universal baseline. */
  globalEnabled: boolean;
  /** Effective state for the selected harness tab. */
  harnessEnabled: boolean;
  /** Forced ON because the skill is enabled in Global. */
  inheritedFromGlobal: boolean;
}

const CATEGORY_ICONS: Record<string, LucideIcon> = {
  Security: ShieldCheck,
  Database: Database,
  Cloud: Cloud,
  Infra: Boxes,
  Frontend: LayoutGrid,
  Backend: Server,
  "AI & Agents": Bot,
  Testing: FlaskConical,
  Docs: FileText,
  Productivity: CalendarDays,
  Media: Image,
  Communication: MessageSquare,
  General: Wrench,
};

export function skillIcon(category: string): LucideIcon {
  return CATEGORY_ICONS[category] ?? Wrench;
}

/** How the skill grid orders cards within the selected scope. */
export type SortMode = "active" | "alpha" | "tokensDesc" | "tokensAsc";

/** Sort choices, in the order the control offers them. */
export const SORT_OPTIONS: { id: SortMode; label: string }[] = [
  { id: "active", label: "Active First" },
  { id: "alpha", label: "Alphabetical (A-Z)" },
  { id: "tokensDesc", label: "Tokens (Highest First)" },
  { id: "tokensAsc", label: "Tokens (Lowest First)" },
];

/** Case-insensitive name order, with an id tie-break so ties never shuffle. */
function compareNames(left: SkillItem, right: SkillItem): number {
  const byName = left.name.localeCompare(right.name, undefined, {
    sensitivity: "base",
  });
  return byName !== 0 ? byName : left.id.localeCompare(right.id);
}

/**
 * Order the grid. `active` floats every enabled skill above the rest (A–Z
 * inside each group). Turning one off moves the card into the idle group;
 * the grid must not scroll to follow it.
 */
export function sortSkills(items: SkillItem[], mode: SortMode): SkillItem[] {
  const sorted = [...items];

  switch (mode) {
    case "alpha":
      return sorted.sort(compareNames);
    case "tokensDesc":
      return sorted.sort((a, b) => b.tokens - a.tokens || compareNames(a, b));
    case "tokensAsc":
      return sorted.sort((a, b) => a.tokens - b.tokens || compareNames(a, b));
    case "active":
    default:
      return sorted.sort(
        (a, b) =>
          Number(b.harnessEnabled) - Number(a.harnessEnabled) ||
          compareNames(a, b),
      );
  }
}

export const vaultPath = "~/.orquestra/skills-library";
export const projectionPath = "~/.orquestra/active";
