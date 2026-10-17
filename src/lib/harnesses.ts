import type { LucideIcon } from "lucide-react";
import {
  Bot,
  Boxes,
  Compass,
  Globe,
  Moon,
  Package,
  Rocket,
  Target,
  Terminal,
  Zap,
} from "lucide-react";

export interface HarnessScope {
  id: string;
  label: string;
  hint: string;
  icon: LucideIcon;
}

export const GLOBAL_SCOPE: HarnessScope = {
  id: "global",
  label: "Global",
  hint: "Universal baseline inherited by every harness",
  icon: Globe,
};

export const HARNESSES: HarnessScope[] = [
  {
    id: "antigravity",
    label: "Antigravity",
    hint: "~/.gemini/config/skills",
    icon: Rocket,
  },
  {
    id: "codex",
    label: "Codex",
    hint: "~/.codex/skills",
    icon: Terminal,
  },
  {
    id: "claude",
    label: "Claude Code",
    hint: "~/.claude/skills",
    icon: Bot,
  },
  {
    id: "freebuff",
    label: "Freebuff",
    hint: "~/.freebuff/worktrees",
    icon: Zap,
  },
  {
    id: "opencode",
    label: "OpenCode",
    hint: "~/.config/opencode",
    icon: Boxes,
  },
  {
    id: "kilo",
    label: "Kilo Code",
    hint: "~/.config/kilo",
    icon: Package,
  },
  {
    id: "copilot",
    label: "Copilot CLI",
    hint: "~/.copilot/skills",
    icon: Target,
  },
  {
    id: "hermes",
    label: "Hermes Agent",
    hint: "~/.hermes/skills",
    icon: Moon,
  },
  {
    id: "grok",
    label: "xAI Grok",
    hint: "~/.grok/skills",
    icon: Compass,
  },
];

/** Display label for a scope id, falling back to the raw id for unknown scopes. */
export function scopeLabel(scope: string): string {
  if (scope === GLOBAL_SCOPE.id) return GLOBAL_SCOPE.label;
  return HARNESSES.find((harness) => harness.id === scope)?.label ?? scope;
}

/** Mirrors the Rust `HarnessSummary` struct. */
export interface HarnessSummary {
  id: string;
  active: number;
  total: number;
}
