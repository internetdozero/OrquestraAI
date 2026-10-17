import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import {
  DEFAULT_ROW_HEIGHT,
  rowOffset,
  rowsFor,
  windowFor,
  type GridWindow,
} from "../lib/virtualRows";

/** Lists up to this size render every card; longer ones are windowed. */
export const VIRTUALIZE_AFTER = 60;

/** Marks the spacer children so they are never measured as cards. */
const SPACER_ATTR = "data-virtual-slot";

/** Fallback cadence for when the compositor never delivers a frame. */
const SCROLL_FALLBACK_MS = 64;

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
  overscanRows = 2,
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
    if (!columnsChanged) {
      const startRow = windowRef.current.start;
      let cardIndex = 0;
      for (const child of Array.from(grid.children)) {
        if (!(child instanceof HTMLElement) || child.hasAttribute(SPACER_ATTR)) {
          continue;
        }
        const height = child.offsetHeight;
        if (height > 0) {
          heightsRef.current.set(
            startRow + Math.floor(cardIndex / measuredColumns),
            height,
          );
        }
        cardIndex += 1;
      }
    }

    const parent = scrollParentOf(grid);
    const gridRect = grid.getBoundingClientRect();
    const viewTop = parent
      ? parent.getBoundingClientRect().top - gridRect.top
      : -gridRect.top;
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

  // Runs after every commit: new cards need measuring before the browser paints.
  useLayoutEffect(() => {
    updateRef.current = update;
    windowRef.current = win;
    update();
  });

  useEffect(() => {
    const grid = gridRef.current;
    if (!virtual || !grid) return;

    const parent = scrollParentOf(grid);
    let frame = 0;
    let timer = 0;

    const run = () => updateRef.current();
    const onFrame = () => {
      frame = 0;
      if (timer) {
        window.clearTimeout(timer);
        timer = 0;
      }
      run();
    };
    // rAF is the fast path; the timer keeps the window honest if frames are
    // suppressed (hidden/minimized window, a stalled compositor).
    const onFallback = () => {
      timer = 0;
      if (frame) {
        window.cancelAnimationFrame(frame);
        frame = 0;
      }
      run();
    };
    const schedule = () => {
      if (frame || timer) return;
      frame = window.requestAnimationFrame(onFrame);
      timer = window.setTimeout(onFallback, SCROLL_FALLBACK_MS);
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
      if (timer) window.clearTimeout(timer);
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
