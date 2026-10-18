/** One-click presets for the most-installed MCP servers. */

export interface McpPreset {
  id: string;
  emoji: string;
  name: string;
  description: string;
  command: string;
  args: string[];
  /** Placeholder env lines shown in the editor (`KEY=` prompts for a value). */
  envTemplate: string;
}

export const MCP_PRESETS: McpPreset[] = [
  {
    id: "github",
    emoji: "🐙",
    name: "GitHub",
    description: "Repositories and issues",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-github"],
    envTemplate: "GITHUB_PERSONAL_ACCESS_TOKEN=",
  },
  {
    id: "postgres",
    emoji: "🐘",
    name: "PostgreSQL",
    description: "Query a database",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-postgres"],
    envTemplate: "DATABASE_URL=postgresql://localhost/mydb",
  },
  {
    id: "filesystem",
    emoji: "📁",
    name: "Filesystem",
    description: "Scoped file access",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem"],
    envTemplate: "ALLOWED_DIR=",
  },
  {
    id: "memory",
    emoji: "🧠",
    name: "Memory",
    description: "Persistent knowledge graph",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-memory"],
    envTemplate: "",
  },
  {
    id: "puppeteer",
    emoji: "🌐",
    name: "Puppeteer",
    description: "Browser automation",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-puppeteer"],
    envTemplate: "",
  },
  {
    id: "brave-search",
    emoji: "🔍",
    name: "Brave Search",
    description: "Web search results",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-brave-search"],
    envTemplate: "BRAVE_API_KEY=",
  },
];
