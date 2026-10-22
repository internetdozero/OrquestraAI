/**
 * Every command in the `generate_handler![…]` registry (src-tauri/src/lib.rs)
 * must have a backticked entry in docs/IPC_API.md, so the protocol
 * documentation cannot drift from the frozen contract.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(fileURLToPath(new URL(".", import.meta.url)), "..");

function registeredCommands() {
  const source = readFileSync(path.join(root, "src-tauri/src/lib.rs"), "utf8");
  const body = source.split("tauri::generate_handler![")[1].split("]")[0];
  return [...new Set(body.split(/[^A-Za-z0-9_]+/).filter(Boolean))].sort();
}

test("every IPC command is documented in docs/IPC_API.md", () => {
  const docs = readFileSync(path.join(root, "docs/IPC_API.md"), "utf8");
  const missing = registeredCommands().filter((command) => !docs.includes(`\`${command}\``));
  assert.deepEqual(missing, []);
});

test("the registry matches the frozen contract command list", () => {
  const fixture = JSON.parse(
    readFileSync(path.join(root, "src-tauri/tests/fixtures/ipc-contract.json"), "utf8"),
  );
  assert.deepEqual(registeredCommands(), [...fixture.commands].sort());
});
