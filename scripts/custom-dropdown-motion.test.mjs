import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const component = readFileSync(
  new URL("../src/components/CustomDropdown.tsx", import.meta.url),
  "utf8",
);
const stylesheet = readFileSync(new URL("../src/index.css", import.meta.url), "utf8");
const listbox = component.match(/<ul\b[^>]*\brole="listbox"[^>]*>/s)?.[0];

function listboxClasses() {
  const value = listbox?.match(/\bclassName="([^"]*)"/)?.[1] ?? "";
  return value.split(/\s+/).filter(Boolean);
}

test("shared dropdown listbox does not use movement animation utilities", () => {
  assert.ok(listbox, "CustomDropdown must keep its shared listbox element");

  const movementUtilities = listboxClasses().filter((name) => {
    const utility = name.split(":").at(-1) ?? name;
    return /^(?:animate(?:-|$)|transition-(?:all|transform)|transform|-?(?:translate|scale|rotate)(?:-|$))/.test(
      utility,
    );
  });

  assert.deepEqual(
    movementUtilities,
    [],
    "opening the shared dropdown must not animate its geometry",
  );
});

test("shared dropdown stylesheet does not restore its old moving entrance animation", () => {
  assert.doesNotMatch(stylesheet, /@keyframes\s+dropdown-in\b|\.dropdown-pop\b/);
});
