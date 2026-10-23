import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  DEFAULT_ROW_HEIGHT,
  GRID_GAP,
  mergeRowHeight,
  rowOffset,
  rowsFor,
  windowFor,
  type GridWindow,
} from "../lib/virtualRows";

/** Lists up to this size render every card; longer ones are windowed. */
export const VIRTUALIZE_AFTER = 60;

/** Marks the spacer children so they are never measured as cards. */
const SPACER_ATTR = "data-virtual-slot";

const EMPTY_WINDOW: GridWindow = {
  start: 0,
  end: 0,
  topSpace: 0,
  bottomSpace: 0,
};

/** Resolved column count of the grid, whatever breakpoint it is at. */
function readColumns(grid: HTMLElement): number {
  const tracks = getComputedStyle(grid).gridTemplateColumns;
  if (!tracks || tracks === "none") return 1;
  return Math.max(1, tracks.split(" ").filter((track) => track.trim() !== "").length);
}

/** Nearest ancestor that actually scrolls vertically. */
function scrollParentOf(node: HTMLElement): HTMLElement | null {
  let current = node.parentElement;
  while (current) {
    if (/(auto|scroll|overlay)/.test(getComputedStyle(current).overflowY)) {
      return current;
    }
    current = current.parentElement;
  }
  return null;
}

function sameWindow(left: GridWindow, right: GridWindow): boolean {
  return (
    left.start === right.start &&
    left.end === right.end &&
    left.topSpace === right.topSpace &&
    left.bottomSpace === right.bottomSpace
  );
}

/**
 * Row-windowed rendering for the card grids.
 *
 * Scroll frames only recompute the window from remembered heights — pure
 * math over two bounding rects, no per-card reads — so fast scrolling never
 * forces a synchronous layout per card. Measuring happens once per commit in
 * the layout effect below, before paint, and the window is recomputed with
 * the fresh heights so the spacers are exact for the frame being painted.
 *
 * `estimate`/`gap` must match the rendered rows: every unvisited row counts
 * the estimate, so a wrong estimate shifts the whole list by the per-row
 * error each time a row resolves to its real height.
 */
export function useVirtualRows({
  itemCount,
  estimate = DEFAULT_ROW_HEIGHT,
  gap = GRID_GAP,
  // Five buffered rows (~15 cards) above and below absorb fast flick-scrolls
  // so no blank void shows before React commits the next frame.
  overscanRows = 5,
  enabled = true,
}: {
  itemCount: number;
  estimate?: number;
  gap?: number;
  overscanRows?: number;
  enabled?: boolean;
}) {
  const gridRef = useRef<HTMLDivElement>(null);
  // `undefined` means unresolved; resolved once, the scroll chain is stable
  // for the life of the grid (unlike per-frame ancestor walks + style reads).
  const parentRef = useRef<HTMLElement | null | undefined>(undefined);
  const heightsRef = useRef(new Map<number, number>());
  const countRef = useRef(itemCount);
  const columnsRef = useRef(1);
  const windowRef = useRef<GridWindow>(EMPTY_WINDOW);
  const updateRef = useRef<() => void>(() => {});
  const [columns, setColumns] = useState(1);
  const [win, setWin] = useState<GridWindow>(EMPTY_WINDOW);

  const virtual = enabled && itemCount > VIRTUALIZE_AFTER;

  const getParent = useCallback((): HTMLElement | null => {
    if (parentRef.current === undefined) {
      const grid = gridRef.current;
      parentRef.current = grid ? scrollParentOf(grid) : null;
    }
    return parentRef.current ?? null;
  }, []);

  const windowFromHeights = useCallback(
    (rowTotal: number): GridWindow | null => {
      const grid = gridRef.current;
      if (!grid) return null;
      const parent = getParent();
      const gridRect = grid.getBoundingClientRect();
      const viewTop = Math.max(
        0,
        parent ? parent.getBoundingClientRect().top - gridRect.top : -gridRect.top,
      );
      const viewHeight = parent ? parent.clientHeight : window.innerHeight;
      return windowFor({
        rowTotal,
        heights: heightsRef.current,
        scrollTop: viewTop,
        viewHeight,
        estimate,
        overscanRows,
        gap,
      });
    },
    [estimate, gap, getParent, overscanRows],
  );

  // Cheap part: window from remembered heights only. No card measurement and
  // no column reads here, so scroll/resize frames stay off the layout path
  // except for the two rects the position math needs.
  const updateWindow = useCallback(() => {
    const rowTotal = rowsFor(itemCount, columns);
    if (!virtual) {
      const full = { start: 0, end: rowTotal, topSpace: 0, bottomSpace: 0 };
      setWin((previous) => (sameWindow(previous, full) ? previous : full));
      return;
    }
    const next = windowFromHeights(rowTotal);
    if (next) setWin((previous) => (sameWindow(previous, next) ? previous : next));
  }, [columns, itemCount, virtual, windowFromHeights]);

  // Expensive part, once per commit and before paint: fold the committed
  // cards into their rows' maxima, then recompute the window with the fresh
  // heights so this frame's spacers already account for what just mounted.
  useLayoutEffect(() => {
    updateRef.current = updateWindow;
    windowRef.current = win;

    const grid = gridRef.current;
    if (!grid) return;

    // New items mean new rows: heights remembered for the old rows would
    // offset the new spacers by the old cards' sizes.
    if (countRef.current !== itemCount) {
      countRef.current = itemCount;
      heightsRef.current.clear();
    }

    const measuredColumns = readColumns(grid);
    if (measuredColumns !== columnsRef.current) {
      // Row indices mean a different set of cards at a new column count (or
      // grid-vs-list switch), so every remembered height is stale.
      heightsRef.current.clear();
      columnsRef.current = measuredColumns;
      setColumns(measuredColumns);
      return;
    }

    if (!virtual) return;

    const startRow = windowRef.current.start;
    const rowMaxHeights = new Map<number, number>();
    let cardIndex = 0;
    for (const child of Array.from(grid.children)) {
      if (!(child instanceof HTMLElement) || child.hasAttribute(SPACER_ATTR)) {
        continue;
      }
      const rowIndex = startRow + Math.floor(cardIndex / measuredColumns);
      rowMaxHeights.set(rowIndex, mergeRowHeight(rowMaxHeights.get(rowIndex), child.offsetHeight));
      cardIndex += 1;
    }
    let changed = false;
    for (const [rowIndex, maxHeight] of rowMaxHeights) {
      if (maxHeight > 0 && heightsRef.current.get(rowIndex) !== maxHeight) {
        heightsRef.current.set(rowIndex, maxHeight);
        changed = true;
      }
    }
    if (changed) {
      const rowTotal = rowsFor(itemCount, measuredColumns);
      const next = windowFromHeights(rowTotal);
      if (next) setWin((previous) => (sameWindow(previous, next) ? previous : next));
    }
    // Runs on every input that can move or resize rows: the scrolled window,
    // the measured column count, the virtualization switch, the item count,
    // and the update callback itself (whose identity tracks estimate/gap).
  }, [updateWindow, win, itemCount, virtual, windowFromHeights]);

  // Initial calculation on mount and whenever the window inputs settle.
  useEffect(() => {
    updateWindow();
  }, [updateWindow]);

  useEffect(() => {
    const grid = gridRef.current;
    if (!virtual || !grid) return;

    let frame = 0;
    const onFrame = () => {
      frame = 0;
      // Resize can move the grid across a breakpoint (or grid-vs-list):
      // recheck the column count here, never on the scroll path, and never
      // synchronously inside the ResizeObserver callback (which runs before
      // paint — forcing style/layout there stalls the compositor mid-resize
      // and shows as a white band on transparent surfaces).
      const current = gridRef.current;
      if (current) {
        const measured = readColumns(current);
        if (measured !== columnsRef.current) {
          heightsRef.current.clear();
          columnsRef.current = measured;
          setColumns(measured);
        }
      }
      updateRef.current();
    };
    const schedule = () => {
      if (frame) return;
      frame = window.requestAnimationFrame(onFrame);
    };
    // Both scroll and resize coalesce into one frame: during a maximize
    // animation the observer fires every frame, and only the latest geometry
    // matters.

    const parent = getParent();
    parent?.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    let observer: ResizeObserver | null = null;
    if (parent) {
      observer = new ResizeObserver(schedule);
      observer.observe(parent);
    }

    return () => {
      parent?.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      observer?.disconnect();
      if (frame) window.cancelAnimationFrame(frame);
    };
  }, [virtual, getParent]);

  /** Bring a row into view when its card is not mounted yet. */
  const scrollToRow = useCallback(
    (row: number) => {
      const grid = gridRef.current;
      if (!grid) return;

      const offset = rowOffset(heightsRef.current, Math.max(0, row), estimate, gap);
      const parent = getParent();

      if (!parent) {
        window.scrollTo({
          top: grid.getBoundingClientRect().top + window.scrollY + offset - 16,
          behavior: "smooth",
        });
        return;
      }

      const gridTopInContent =
        grid.getBoundingClientRect().top - parent.getBoundingClientRect().top + parent.scrollTop;
      parent.scrollTo({
        top: Math.max(0, gridTopInContent + offset - 16),
        behavior: "smooth",
      });
      updateRef.current();
    },
    [estimate, gap, getParent],
  );

  return { gridRef, columns, virtual, rows: win, scrollToRow };
}
