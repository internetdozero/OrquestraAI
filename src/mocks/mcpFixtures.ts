import type { McpServer, McpTarget } from "../lib/mcps";
import type { OfficialMcpEntry } from "../lib/tauri";

/**
 * Developer fixture gallery: a realistic MCP catalog mixing official and
 * community servers, harness targets, and Official Store entries, so the
 * registry grid, sorting, drawers, modals and store tab can be clicked
 * through in a plain browser. Data only — activation and mutation live in
 * `lib/fixtures`.
 */
export const FIXTURE_MCP_SERVERS: McpServer[] = [
  {
    id: "cloudflare",
    name: "Cloudflare Control Plane",
    description:
      "Unified Cloudflare infrastructure management: D1 SQL databases, Workers, KV, R2 storage, DNS records and on-demand documentation.",
    command: "/usr/local/bin/cloudflare-mcp",
    args: ["serve"],
    env: { CLOUDFLARE_API_TOKEN: "cf_mock_token" },
    globalEnabled: true,
    enabledHarnesses: [],
    official: true,
  },
  {
    id: "idea-vault",
    name: "Idea Vault (Local SQLite)",
    description:
      "Local SQLite idea capture plus inter-agent blackboard: record, search and export ideas, post and read async notes between agents.",
    command: "/usr/local/bin/idea-vault-mcp",
    args: [],
    env: {},
    globalEnabled: true,
    enabledHarnesses: [],
    official: true,
  },
  {
    id: "janitor",
    name: "Janitor (System & Memory Guard)",
    description:
      "Linux system health monitoring, RAM/tmpfs OOM prevention, orphan process sweeper and transient cache cleaner.",
    command: "/usr/local/bin/janitor-mcp",
    args: ["--watch"],
    env: {},
    globalEnabled: true,
    enabledHarnesses: [],
    official: true,
  },
  {
    id: "orquestra-core",
    name: "Orquestra Core (Control Plane)",
    description:
      "Just-in-time skills, MCP server toggles, fleet bridge status, master rules and skill/server lifecycle management.",
    command: "/usr/local/bin/orquestra-mcp",
    args: [],
    env: {},
    globalEnabled: true,
    enabledHarnesses: [],
    official: true,
  },
  {
    id: "puppeteer",
    name: "Puppeteer Web Automation",
    description:
      "Headless Chrome automation, web scraping, screenshot capture, DOM interaction and JS evaluation.",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-puppeteer"],
    env: {},
    globalEnabled: true,
    enabledHarnesses: [],
    official: false,
  },
  {
    id: "session-relay",
    name: "Session Relay (Cross-Harness)",
    description:
      "Read-only inspection of Codex, Claude Code, Antigravity, OpenCode and Freebuff sessions, with keyword search and an executive handoff.",
    command: "/usr/local/bin/session-relay-mcp",
    args: [],
    env: {},
    globalEnabled: true,
    enabledHarnesses: [],
    official: true,
  },
  {
    id: "github",
    name: "GitHub MCP Server",
    description: "Repos, issues, pull requests and code search over the GitHub API.",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-github"],
    env: { GITHUB_PERSONAL_ACCESS_TOKEN: "ghp_mocktoken" },
    globalEnabled: false,
    enabledHarnesses: ["codex", "claude"],
    official: false,
  },
  {
    id: "canva",
    name: "Canva Apps SDK",
    description: "Generate and preview Canva app scaffolds from a design prompt.",
    command: "npx",
    args: ["-y", "@canva/app-mcp"],
    env: {},
    globalEnabled: false,
    enabledHarnesses: [],
    official: false,
  },
  {
    id: "headroom",
    name: "Headroom Token Compressor",
    description: "Summarize bulky tool outputs before they reach the context window.",
    command: "python3",
    args: ["-m", "headroom_mcp"],
    env: { HEADROOM_RATIO: "0.4" },
    globalEnabled: false,
    enabledHarnesses: ["opencode"],
    official: false,
  },
  {
    id: "web-fetch",
    name: "Web Fetch (HTTP)",
    description: "Fetch URLs over HTTP with optional bearer auth and header overrides.",
    command: "/usr/local/bin/webfetch-mcp",
    args: ["--timeout", "20"],
    env: {},
    url: "https://mcp.example.com/fetch",
    headers: { Authorization: "Bearer mock-bearer" },
    globalEnabled: false,
    enabledHarnesses: [],
    official: false,
  },
];

/** Harness adapters the scope selector can project the catalog onto. */
export const FIXTURE_MCP_TARGETS: McpTarget[] = [
  {
    id: "codex",
    label: "Codex",
    configPath: "~/.codex/config.toml",
    installed: true,
  },
  {
    id: "claude",
    label: "Claude Code",
    configPath: "~/.claude.json",
    installed: true,
  },
  {
    id: "opencode",
    label: "OpenCode",
    configPath: "~/.config/opencode/opencode.json",
    installed: true,
  },
  {
    id: "antigravity",
    label: "Antigravity",
    configPath: "~/.gemini/antigravity.json",
    installed: false,
  },
];

/**
 * Curated store shelf: `github` matches the catalog (toggle demo),
 * `postgres` is missing (Add demo), the rest are already active.
 */
export const FIXTURE_OFFICIAL_MCPS: OfficialMcpEntry[] = [
  {
    id: "github",
    name: "GitHub MCP Server",
    description: "Repos, issues, pull requests and code search over the GitHub API.",
    repoUrl: "https://github.com/modelcontextprotocol/servers",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-github"],
  },
  {
    id: "postgres",
    name: "PostgreSQL MCP Server",
    description: "Read-only SQL exploration with schema inspection and query guardrails.",
    repoUrl: "https://github.com/modelcontextprotocol/servers",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/dev"],
  },
  {
    id: "cloudflare",
    name: "Cloudflare Control Plane",
    description: "D1, Workers, KV, R2, DNS and on-demand documentation.",
    command: "/usr/local/bin/cloudflare-mcp",
    args: ["serve"],
  },
  {
    id: "puppeteer",
    name: "Puppeteer Web Automation",
    description: "Headless Chrome automation, scraping and screenshots.",
    repoUrl: "https://github.com/modelcontextprotocol/servers",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-puppeteer"],
  },
];
