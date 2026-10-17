/**
 * Geometry tests for `src/lib/virtualRows.ts`.
 *
 * The grid virtualization is only correct if the two spacers plus the rendered
 * rows reproduce the exact height of the full grid. That invariant is asserted
 * for every case here, because a spacer that is off by one gap shows up as a
 * small jump at the end of the list in the real app.
 *
 * Run: `npm test` (Node strips the TypeScript types natively).
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_ROW_HEIGHT,
  GRID_GAP,
  rowHeightAt,
  rowOffset,
  rowsFor,
  totalHeight,
  windowFor,
} from "../src/lib/virtualRows.ts";

/** Compose a window the way the DOM does: spacer, rows, spacer. */
function renderedHeight(window, heights, estimate = DEFAULT_ROW_HEIGHT) {
  let middle = 0;
  for (let row = window.start; row < window.end; row += 1) {
    middle += rowHeightAt(heights, row, estimate);
  }
  const rows = window.end - window.start;
  const gaps = rows - 1 + (window.topSpace > 0 ? 1 : 0) + (window.bottomSpace > 0 ? 1 : 0);
  return window.topSpace + middle + window.bottomSpace + gaps * GRID_GAP;
}

test("rowsFor chunks items into complete rows", () => {
  assert.equal(rowsFor(0, 3), 0);
  assert.equal(rowsFor(1, 3), 1);
  assert.equal(rowsFor(3, 3), 1);
  assert.equal(rowsFor(7, 3), 3);
  assert.equal(rowsFor(7, 1), 7);
  assert.equal(rowsFor(10, 0), 0);
});

test("unmeasured rows use the estimate, measured rows win", () => {
  const heights = new Map([[2, 240]]);
  assert.equal(rowHeightAt(heights, 0), DEFAULT_ROW_HEIGHT);
  assert.equal(rowHeightAt(heights, 2), 240);
  assert.equal(rowHeightAt(new Map([[1, 0]]), 1), DEFAULT_ROW_HEIGHT);
});

test("offsets count the gap after each earlier row", () => {
  const heights = new Map([
    [0, 100],
    [1, 120],
  ]);
  assert.equal(rowOffset(heights, 0), 0);
  assert.equal(rowOffset(heights, 1), 112);
  assert.equal(rowOffset(heights, 2), 244);
  assert.equal(totalHeight(2, heights), 232);
  assert.equal(totalHeight(0, heights), 0);
});

test("a window at the top renders the first rows and only a bottom spacer", () => {
  const heights = new Map();
  const window = windowFor({
    rowTotal: 100,
    heights,
    scrollTop: 0,
    viewHeight: 600,
    overscanRows: 1,
  });

  assert.equal(window.start, 0);
  assert.equal(window.topSpace, 0);
  // 600px of viewport at a 188px row pitch is 4 rows, plus one overscan row.
  assert.equal(window.end, 5);
  assert.ok(window.bottomSpace > 0);
});

test("scrolling keeps the target row inside the window", () => {
  const heights = new Map();
  const window = windowFor({
    rowTotal: 100,
    heights,
    scrollTop: rowOffset(heights, 80),
    viewHeight: 600,
    overscanRows: 2,
  });

  assert.ok(window.start <= 80, `row 80 must not start after the window: ${window.start}`);
  assert.ok(window.end > 80, `row 80 must be rendered: ${window.end}`);
  assert.ok(window.topSpace > 0);
  assert.ok(window.bottomSpace > 0);
});

test("spacers reproduce the full grid height for every viewport", () => {
  const heights = new Map();
  for (let row = 0; row < 30; row += 1) {
    // Deliberately uneven rows, as real cards are.
    heights.set(row, row % 3 === 0 ? 140 : 196);
  }

  for (const viewHeight of [400, 600, 1200]) {
    for (const scrollTop of [0, 500, 2500, 9000]) {
      const window = windowFor({
        rowTotal: 30,
        heights,
        scrollTop,
        viewHeight,
        overscanRows: 2,
      });
      const expected = totalHeight(30, heights);
      const actual = renderedHeight(window, heights);

      assert.equal(
        actual,
        expected,
        `viewport ${viewHeight} at ${scrollTop}: spacers sum to ${actual}, grid is ${expected}`,
      );
      assert.ok(window.start >= 0 && window.start <= window.end);
      assert.ok(window.end <= 30);
      assert.ok(window.topSpace >= 0 && window.bottomSpace >= 0);
    }
  }
});

test("an empty grid produces no spacers", () => {
  const window = windowFor({
    rowTotal: 0,
    heights: new Map(),
    scrollTop: 0,
    viewHeight: 600,
  });

  assert.deepEqual(window, { start: 0, end: 0, topSpace: 0, bottomSpace: 0 });
});

test("the window clamps to the last row and drops the bottom spacer", () => {
  const heights = new Map();
  const window = windowFor({
    rowTotal: 10,
    heights,
    scrollTop: totalHeight(10, heights) - 300,
    viewHeight: 300,
    overscanRows: 2,
  });

  assert.equal(window.end, 10);
  assert.equal(window.bottomSpace, 0);
  assert.ok(window.start > 0);
});
