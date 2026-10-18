/** Smart paste: turn a pasted CLI string or JSON block into MCP fields. */

export interface ParsedMcpCommand {
  command: string;
  args: string[];
  env: Record<string, string>;
  url?: string;
}

function splitCli(text: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let quote: string | null = null;

  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (quote) {
      if (ch === quote) quote = null;
      else current += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
    } else if (/\s/.test(ch)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
    } else {
      current += ch;
    }
  }
  if (current) tokens.push(current);
  return tokens;
}

/** `KEY=val cmd ...` prefixes become env; the rest is the command line. */
function splitEnvPrefix(tokens: string[]): { env: Record<string, string>; rest: string[] } {
  const env: Record<string, string> = {};
  let i = 0;
  while (i < tokens.length && /^[A-Za-z_][A-Za-z0-9_]*=/.test(tokens[i])) {
    const split = tokens[i].indexOf("=");
    env[tokens[i].slice(0, split)] = tokens[i].slice(split + 1);
    i += 1;
  }
  return { env, rest: tokens.slice(i) };
}

function fromCli(text: string): ParsedMcpCommand | null {
  const tokens = splitCli(text.trim());
  if (tokens.length === 0) return null;
  // A bare `npx` with no package is not a server definition.
  const cleaned = tokens[0] === "$" || tokens[0] === ">" ? tokens.slice(1) : tokens;
  if (cleaned.length < 2) return null;
  const { env, rest } = splitEnvPrefix(cleaned);
  if (rest.length < 2) return null;
  return { command: rest[0], args: rest.slice(1), env };
}

function recordOf(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function stringArray(value: unknown): string[] | null {
  if (!Array.isArray(value) || !value.every((item) => typeof item === "string")) return null;
  return value as string[];
}

function stringRecord(value: unknown): Record<string, string> | null {
  const record = recordOf(value);
  if (!record || !Object.values(record).every((item) => typeof item === "string")) return null;
  return record as Record<string, string>;
}

function fromServerObject(obj: Record<string, unknown>): ParsedMcpCommand | null {
  const command = typeof obj.command === "string" ? obj.command.trim() : "";
  if (!command) return null;
  const args = stringArray(obj.args) ?? [];
  const env = stringRecord(obj.env) ?? {};
  const url = typeof obj.url === "string" && obj.url.trim() ? obj.url.trim() : undefined;
  return { command, args, env, url };
}

function fromJson(text: string): ParsedMcpCommand | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }
  const root = recordOf(parsed);
  if (!root) return null;

  // Direct server shape: `{ "command": "npx", "args": [...], "env": {...} }`.
  const direct = fromServerObject(root);
  if (direct) return direct;

  // Claude wrapper: `{ "mcpServers": { "<name>": { ... } } }` — first entry wins.
  const servers = recordOf(root.mcpServers);
  if (servers) {
    for (const entry of Object.values(servers)) {
      const candidate = recordOf(entry);
      if (candidate) {
        const parsedServer = fromServerObject(candidate);
        if (parsedServer) return parsedServer;
      }
    }
  }
  return null;
}

/**
 * Parse a smart-paste blob. JSON is tried first (Claude blocks and raw
 * server objects); anything else is read as a CLI string with optional
 * `KEY=val` env prefixes.
 */
export function parseSmartMcpCommand(text: string): ParsedMcpCommand | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("{")) {
    return fromJson(trimmed) ?? fromCli(trimmed);
  }
  return fromCli(trimmed);
}
