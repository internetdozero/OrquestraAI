/**
 * Tests for `shortenPath` in `src/lib/bridge.ts`.
 *
 * Verifies cross-platform path shortening:
 * - Linux: /home/<user>/...
 * - macOS: /Users/<user>/...
 * - Windows: C:\Users\<user>\... and C:/Users/<user>/...
 * - Backend-supplied homePrefix override
 */
import test from "node:test";
import assert from "node:assert/strict";
import { shortenPath } from "../src/lib/bridge.ts";

test("shortenPath on Linux paths", () => {
  assert.equal(shortenPath("/home/user/.agents/mcp.json"), "~/.agents/mcp.json");
  assert.equal(shortenPath("/home/user"), "~");
  assert.equal(shortenPath("/home/alice/nested/deep/file.txt"), "~/nested/deep/file.txt");
});

test("shortenPath on macOS paths", () => {
  assert.equal(
    shortenPath("/Users/developer/Library/Application Support/opencode.json"),
    "~/Library/Application Support/opencode.json",
  );
  assert.equal(shortenPath("/Users/developer"), "~");
});

test("shortenPath on Windows paths", () => {
  assert.equal(
    shortenPath("C:\\Users\\admin\\.agents\\mcp.json"),
    "~/.agents/mcp.json",
  );
  assert.equal(
    shortenPath("C:/Users/admin/.agents/mcp.json"),
    "~/.agents/mcp.json",
  );
  assert.equal(shortenPath("D:\\Users\\developer"), "~");
  assert.equal(
    shortenPath("E:\\Users\\alice\\AppData\\Local\\Orquestra\\state.json"),
    "~/AppData/Local/Orquestra/state.json",
  );
});

test("shortenPath with backend-supplied homePrefix", () => {
  assert.equal(
    shortenPath("/mnt/storage/custom-user/.config/test.json", "/mnt/storage/custom-user"),
    "~/.config/test.json",
  );
  assert.equal(
    shortenPath("D:\\Profile\\user\\app.cfg", "D:\\Profile\\user"),
    "~/app.cfg",
  );
  assert.equal(
    shortenPath("/mnt/storage/custom-user", "/mnt/storage/custom-user"),
    "~",
  );
});

test("shortenPath preserves non-home paths", () => {
  assert.equal(shortenPath("/etc/systemd/system"), "/etc/systemd/system");
  assert.equal(shortenPath("/tmp/scratch.log"), "/tmp/scratch.log");
  assert.equal(shortenPath("C:\\Windows\\System32"), "C:\\Windows\\System32");
});
