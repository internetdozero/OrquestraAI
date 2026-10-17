import { FileText, Layers, Plug, type LucideIcon } from "lucide-react";

/**
 * Top-level navigation of the shell. Harness scopes are *not* views: they are
 * tabs inside the skills view, the same way the MCP registry already presents
 * Global and the harnesses as one screen with an inner scope selector.
 */
export type ActiveView = "skills" | "mcps" | "agents";

export interface ViewDefinition {
  id: ActiveView;
  label: string;
  icon: LucideIcon;
  hint: string;
}

export const VIEWS: ViewDefinition[] = [
  {
    id: "skills",
    label: "Skills",
    icon: Layers,
    hint: "Skill projections, per harness scope",
  },
  {
    id: "mcps",
    label: "MCP Servers",
    icon: Plug,
    hint: "MCP servers compiled into every harness",
  },
  {
    id: "agents",
    label: "AGENTS.md",
    icon: FileText,
    hint: "Master rules projected into every harness",
  },
];
