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
  LIST_GAP,
  mergeRowHeight,
  rowHeightAt,
  rowOffset,
  rowsFor,
  totalHeight,
  windowFor,
} from "../src/lib/virtualRows.ts";

/** Compose a window the way the DOM does: spacer, rows, spacer. */
function renderedHeight(window, heights, estimate = DEFAULT_ROW_HEIGHT, gap = GRID_GAP) {
  let middle = 0;
  for (let row = window.start; row < window.end; row += 1) {
    middle += rowHeightAt(heights, row, estimate);
  }
  const rows = window.end - window.start;
  const gaps = rows - 1 + (window.topSpace > 0 ? 1 : 0) + (window.bottomSpace > 0 ? 1 : 0);
  return window.topSpace + middle + window.bottomSpace + gaps * gap;
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
  // 600px of viewport at a 216px row pitch is 3 rows, plus one overscan row.
  assert.equal(window.end, 4);
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

test("mergeRowHeight keeps the tallest card of the row", () => {
  // Tall card measured first must survive shorter siblings measured later.
  assert.equal(mergeRowHeight(mergeRowHeight(undefined, 300), 140), 300);
  // Tall card measured last must still win over earlier short cards.
  assert.equal(mergeRowHeight(mergeRowHeight(mergeRowHeight(undefined, 140), 150), 300), 300);
  // Equal heights are stable.
  assert.equal(mergeRowHeight(176, 176), 176);
  // Hidden or unlaid-out cards (zero/negative reads) never clobber a max.
  assert.equal(mergeRowHeight(240, 0), 240);
  assert.equal(mergeRowHeight(240, -10), 240);
  assert.equal(mergeRowHeight(undefined, 0), 0);
});

test("folding per-card heights yields the row maximum", () => {
  // Mirrors the hook's measurement loop: three cards in one CSS grid row
  // with uneven heights; the stored row height must equal the tallest.
  const columns = 3;
  const startRow = 4;
  const cardHeights = [140, 300, 152, 176, 176, 210];
  const rowMaxHeights = new Map();
  cardHeights.forEach((height, cardIndex) => {
    const rowIndex = startRow + Math.floor(cardIndex / columns);
    rowMaxHeights.set(rowIndex, mergeRowHeight(rowMaxHeights.get(rowIndex), height));
  });
  assert.equal(rowMaxHeights.get(4), 300);
  assert.equal(rowMaxHeights.get(5), 210);
  // A re-measure where the tallest card now reads short keeps working
  // through max-folding: the stored value only ever reflects measured maxes.
  const remeasured = new Map();
  [120, 130, 125].forEach((height, cardIndex) => {
    const rowIndex = startRow + Math.floor(cardIndex / columns);
    remeasured.set(rowIndex, mergeRowHeight(remeasured.get(rowIndex), height));
  });
  assert.equal(remeasured.get(4), 130);
});

test("spacers account for irregular tallest-item row heights", () => {
  // Rows whose heights came from tallest-item folding, not uniform cards.
  const heights = new Map([
    [0, 300],
    [1, 140],
    [2, 252],
    [3, 176],
    [4, 320],
    [5, 141],
  ]);
  for (const viewHeight of [400, 600]) {
    for (const scrollTop of [0, 300, 1200]) {
      const window = windowFor({
        rowTotal: 6,
        heights,
        scrollTop,
        viewHeight,
        overscanRows: 1,
      });
      assert.equal(
        renderedHeight(window, heights),
        totalHeight(6, heights),
        `viewport ${viewHeight} at ${scrollTop}: spacers must reproduce the jagged grid`,
      );
    }
  }
});

test("a custom gap reproduces the full list height", () => {
  // List rows use gap-2 (8px), not the grid's gap-3: spacers must account
  // with the list gap or every row drifts the scroll by the difference.
  const heights = new Map([
    [0, 57],
    [1, 57],
    [2, 57],
  ]);
  assert.equal(totalHeight(3, heights, DEFAULT_ROW_HEIGHT, LIST_GAP), 57 * 3 + LIST_GAP * 2);
  assert.equal(rowOffset(heights, 2, DEFAULT_ROW_HEIGHT, LIST_GAP), 57 * 2 + LIST_GAP * 2);
  const window = windowFor({
    rowTotal: 30,
    heights: new Map(),
    scrollTop: 500,
    viewHeight: 600,
    overscanRows: 2,
    gap: LIST_GAP,
  });
  assert.equal(
    renderedHeight(window, heights, DEFAULT_ROW_HEIGHT, LIST_GAP),
    totalHeight(30, heights, DEFAULT_ROW_HEIGHT, LIST_GAP),
  );
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
