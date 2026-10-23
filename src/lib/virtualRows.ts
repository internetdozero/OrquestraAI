/**
 * Pure geometry for row-windowed card grids.
 *
 * The grids are CSS grids (1/2/3 columns by breakpoint), so the unit of
 * virtualization is a *row*: a row is `columns` consecutive cards, and every
 * rendered row is complete, which keeps the CSS grid placing cards from column
 * one. Everything here is pure so it can be tested without a DOM.
 */

/** Vertical gap between card rows; matches `gap-3` in the grid classes. */
export const GRID_GAP = 12;

/** Vertical gap between list rows; matches `gap-2` in the list classes. */
export const LIST_GAP = 8;

/**
 * Height assumed for a row until it has been measured on screen. Skill cards
 * measure ~188–212px (avg ~198px), so the estimate keeps the cumulative
 * spacer delta near zero as unvisited rows enter the viewport. Views whose
 * rows differ (list rows, MCP cards) pass their own estimate instead.
 */
export const DEFAULT_ROW_HEIGHT = 198;

export interface GridWindow {
  /** First rendered row (inclusive). */
  start: number;
  /** Last rendered row (exclusive). */
  end: number;
  /** Spacer height standing in for every row before `start`. */
  topSpace: number;
  /** Spacer height standing in for every row from `end` on. */
  bottomSpace: number;
}

/** Number of card rows for `itemCount` items laid out in `columns` columns. */
export function rowsFor(itemCount: number, columns: number): number {
  if (itemCount <= 0 || columns <= 0) return 0;
  return Math.ceil(itemCount / columns);
}

/**
 * Fold one measured card height into its row's remembered maximum.
 *
 * CSS grid stretches every track to its tallest item, so a row's height is
 * the max of its cards — never the last card measured. Non-positive reads
 * (hidden or unlaid-out cards) leave the remembered value untouched.
 */
export function mergeRowHeight(current: number | undefined, height: number): number {
  if (!(height > 0)) return current ?? 0;
  return Math.max(current ?? 0, height);
}

/** Measured height of a row, falling back to the estimate when unknown. */
export function rowHeightAt(
  heights: ReadonlyMap<number, number>,
  row: number,
  estimate: number = DEFAULT_ROW_HEIGHT,
): number {
  const measured = heights.get(row);
  return measured !== undefined && measured > 0 ? measured : estimate;
}

/**
 * Offset of the top of `row` in grid content coordinates. Gaps live between
 * rows, so a row's offset counts the gap that follows each earlier row.
 * `gap` must match the container's CSS gap, otherwise every unvisited row
 * drifts the scroll position by the difference (list rows with a grid gap
 * visibly jump as they enter the viewport).
 */
export function rowOffset(
  heights: ReadonlyMap<number, number>,
  row: number,
  estimate: number = DEFAULT_ROW_HEIGHT,
  gap: number = GRID_GAP,
): number {
  let offset = 0;
  for (let index = 0; index < row; index += 1) {
    offset += rowHeightAt(heights, index, estimate) + gap;
  }
  return offset;
}

/** Total height of every row, gaps included, with no trailing gap. */
export function totalHeight(
  rowTotal: number,
  heights: ReadonlyMap<number, number>,
  estimate: number = DEFAULT_ROW_HEIGHT,
  gap: number = GRID_GAP,
): number {
  if (rowTotal <= 0) return 0;
  return Math.max(0, rowOffset(heights, rowTotal, estimate, gap) - gap);
}

/**
 * Which rows to render for a viewport that starts at `scrollTop` (grid content
 * coordinates). Rows are measured when visited, so heights above the viewport
 * are exact once scrolled past; unseen rows fall back to `estimate`.
 *
 * Spacer heights are the space the hidden rows would occupy, minus one gap:
 * the spacer itself sits in the grid flow and already receives a gap.
 */
export function windowFor({
  rowTotal,
  heights,
  scrollTop,
  viewHeight,
  estimate = DEFAULT_ROW_HEIGHT,
  overscanRows = 2,
  gap = GRID_GAP,
}: {
  rowTotal: number;
  heights: ReadonlyMap<number, number>;
  scrollTop: number;
  viewHeight: number;
  estimate?: number;
  overscanRows?: number;
  gap?: number;
}): GridWindow {
  if (rowTotal <= 0) {
    return { start: 0, end: 0, topSpace: 0, bottomSpace: 0 };
  }

  const viewBottom = scrollTop + Math.max(viewHeight, 0);

  // First row whose bottom edge is at or below the top of the viewport.
  let start = 0;
  let rowTop = 0;
  while (start < rowTotal) {
    const height = rowHeightAt(heights, start, estimate);
    if (rowTop + height >= scrollTop) break;
    rowTop += height + gap;
    start += 1;
  }

  // Last row that begins above the bottom edge of the viewport.
  let end = start;
  let cursor = rowTop;
  while (end < rowTotal && cursor <= viewBottom) {
    cursor += rowHeightAt(heights, end, estimate) + gap;
    end += 1;
  }

  start = Math.max(0, start - overscanRows);
  end = Math.min(rowTotal, Math.max(end, start + 1) + overscanRows);

  let topSpace = 0;
  for (let row = 0; row < start; row += 1) {
    topSpace += rowHeightAt(heights, row, estimate) + gap;
  }
  if (start > 0) topSpace = Math.max(0, topSpace - gap);

  let bottomSpace = 0;
  for (let row = end; row < rowTotal; row += 1) {
    bottomSpace += rowHeightAt(heights, row, estimate) + gap;
  }
  if (end < rowTotal) bottomSpace = Math.max(0, bottomSpace - gap);

  return { start, end, topSpace, bottomSpace };
}
