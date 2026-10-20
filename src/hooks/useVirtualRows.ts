import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import {
  DEFAULT_ROW_HEIGHT,
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
  return Math.max(
    1,
    tracks.split(" ").filter((track) => track.trim() !== "").length,
  );
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
 * Rows are measured from the DOM after every commit and remembered, so the
 * spacers above the window always add up to the real height of the rows they
 * stand in for — scrolling past a row never shifts content retroactively. Rows
 * that have never been on screen fall back to `estimate`, which only affects
 * the height of the list below the viewport.
 */
export function useVirtualRows({
  itemCount,
  estimate = DEFAULT_ROW_HEIGHT,
  // Five buffered rows (~15 cards) above and below absorb fast flick-scrolls
  // so no blank void shows before React commits the next frame.
  overscanRows = 5,
  enabled = true,
}: {
  itemCount: number;
  estimate?: number;
  overscanRows?: number;
  enabled?: boolean;
}) {
  const gridRef = useRef<HTMLDivElement>(null);
  const heightsRef = useRef(new Map<number, number>());
  const windowRef = useRef<GridWindow>(EMPTY_WINDOW);
  const updateRef = useRef<() => void>(() => {});
  const [columns, setColumns] = useState(1);
  const [win, setWin] = useState<GridWindow>(EMPTY_WINDOW);

  const virtual = enabled && itemCount > VIRTUALIZE_AFTER;

  const update = useCallback(() => {
    const grid = gridRef.current;
    if (!grid) return;

    const measuredColumns = readColumns(grid);
    const columnsChanged = measuredColumns !== columns;
    if (columnsChanged) {
      // Row indices mean a different set of cards at a new column count, so
      // every remembered height is stale. Estimates carry this one frame.
      heightsRef.current.clear();
      setColumns(measuredColumns);
    }

    const rowTotal = rowsFor(itemCount, measuredColumns);

    if (!virtual) {
      heightsRef.current.clear();
      const full = { start: 0, end: rowTotal, topSpace: 0, bottomSpace: 0 };
      setWin((previous) => (sameWindow(previous, full) ? previous : full));
      return;
    }

    // Every rendered row is complete, so card n belongs to row `start + n/cols`.
    // A grid track is as tall as its tallest item, so fold every card of a
    // row into that row's maximum instead of letting the last measured card
    // overwrite the earlier (possibly taller) ones.
    if (!columnsChanged) {
      const startRow = windowRef.current.start;
      const rowMaxHeights = new Map<number, number>();
      let cardIndex = 0;
      for (const child of Array.from(grid.children)) {
        if (!(child instanceof HTMLElement) || child.hasAttribute(SPACER_ATTR)) {
          continue;
        }
        const rowIndex = startRow + Math.floor(cardIndex / measuredColumns);
        rowMaxHeights.set(
          rowIndex,
          mergeRowHeight(rowMaxHeights.get(rowIndex), child.offsetHeight),
        );
        cardIndex += 1;
      }
      for (const [rowIndex, maxHeight] of rowMaxHeights) {
        if (maxHeight > 0) heightsRef.current.set(rowIndex, maxHeight);
      }
    }

    const parent = scrollParentOf(grid);
    const gridRect = grid.getBoundingClientRect();
    const viewTop = Math.max(
      0,
      parent
        ? parent.getBoundingClientRect().top - gridRect.top
        : -gridRect.top,
    );
    const viewHeight = parent ? parent.clientHeight : window.innerHeight;

    const next = windowFor({
      rowTotal,
      heights: heightsRef.current,
      scrollTop: viewTop,
      viewHeight,
      estimate,
      overscanRows,
    });

    setWin((previous) => (sameWindow(previous, next) ? previous : next));
  }, [columns, estimate, itemCount, overscanRows, virtual]);

  // Measures rendered cards from the DOM after commit so heightsRef is always
  // accurate for the next scroll frame, without re-triggering setWin synchronously.
  useLayoutEffect(() => {
    updateRef.current = update;
    windowRef.current = win;

    const grid = gridRef.current;
    if (!grid) return;

    const measuredColumns = readColumns(grid);
    if (measuredColumns !== columns) {
      heightsRef.current.clear();
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
      rowMaxHeights.set(
        rowIndex,
        mergeRowHeight(rowMaxHeights.get(rowIndex), child.offsetHeight),
      );
      cardIndex += 1;
    }
    for (const [rowIndex, maxHeight] of rowMaxHeights) {
      if (maxHeight > 0) heightsRef.current.set(rowIndex, maxHeight);
    }
  });

  // Initial calculation on mount and whenever column count stabilizes.
  useEffect(() => {
    update();
  }, [update]);

  useEffect(() => {
    const grid = gridRef.current;
    if (!virtual || !grid) return;

    const parent = scrollParentOf(grid);
    let frame = 0;

    const onFrame = () => {
      frame = 0;
      updateRef.current();
    };
    const schedule = () => {
      if (frame) return;
      frame = window.requestAnimationFrame(onFrame);
    };

    let lastSize = "";
    const observer = new ResizeObserver(() => {
      const size = `${parent?.clientWidth ?? 0}x${parent?.clientHeight ?? 0}`;
      if (size === lastSize) return;
      lastSize = size;
      updateRef.current();
    });

    parent?.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    if (parent) observer.observe(parent);

    return () => {
      parent?.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      observer.disconnect();
      if (frame) window.cancelAnimationFrame(frame);
    };
  }, [virtual]);

  /** Bring a row into view when its card is not mounted yet. */
  const scrollToRow = useCallback(
    (row: number) => {
      const grid = gridRef.current;
      if (!grid) return;

      const offset = rowOffset(heightsRef.current, Math.max(0, row), estimate);
      const parent = scrollParentOf(grid);

      if (!parent) {
        window.scrollTo({
          top: grid.getBoundingClientRect().top + window.scrollY + offset - 16,
          behavior: "smooth",
        });
        return;
      }

      const gridTopInContent =
        grid.getBoundingClientRect().top -
        parent.getBoundingClientRect().top +
        parent.scrollTop;
      parent.scrollTo({
        top: Math.max(0, gridTopInContent + offset - 16),
        behavior: "smooth",
      });
      updateRef.current();
    },
    [estimate],
  );

  return { gridRef, columns, virtual, rows: win, scrollToRow };
}
