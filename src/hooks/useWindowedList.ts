import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

/**
 * Windowing for variable-height vertical lists (chat transcripts, feeds).
 *
 * Only the rows around the viewport (plus an overscan buffer) stay mounted;
 * everything above/below collapses into measured spacer divs. Row heights are
 * cached from the DOM after each commit and fall back to `estimate` until
 * measured, so the scrollbar stays stable and no scroll jump occurs when
 * rows mount. The parent renders:
 *
 *   <div ref={rootRef}>
 *     {topSpacer > 0 && <div style={{ height: topSpacer }} aria-hidden="true" />}
 *     {items.slice(start, end).map((item, i) => (
 *       <div key={...} data-index={start + i} className="pb-2">…</div>
 *     ))}
 *     {bottomSpacer > 0 && <div style={{ height: bottomSpacer }} aria-hidden="true" />}
 *   </div>
 *
 * Item wrappers must carry `data-index` (position in the full list) and
 * include their own gap (e.g. `pb-2`, which `offsetHeight` covers) instead
 * of relying on the container's `space-y`, which would also space the
 * spacer divs and break the math. `resetKey` should change whenever the list
 * identity changes (session switch, new filter) to drop cached heights.
 */
export function useWindowedList(
  itemCount: number,
  resetKey: string,
  estimate = 120,
  overscanPx = 800,
  initial = 60,
) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const heightsRef = useRef(new Map<number, number>());
  const [range, setRange] = useState(() => ({
    start: 0,
    end: Math.min(initial, itemCount),
    topSpacer: 0,
    bottomSpacer: 0,
  }));

  // Fresh list (session switch, new filter text): drop cached heights and
  // render from the top; the scroll listener re-seats the window below.
  useEffect(() => {
    heightsRef.current.clear();
    setRange({
      start: 0,
      end: Math.min(initial, itemCount),
      topSpacer: 0,
      bottomSpacer: 0,
    });
  }, [resetKey, itemCount, initial]);

  /** Measure mounted rows, then seat [start, end) over the viewport. */
  const refresh = useCallback(() => {
    const root = rootRef.current;
    if (!root || itemCount === 0) return;

    for (const child of Array.from(root.children)) {
      if (!(child instanceof HTMLElement)) continue;
      const raw = child.getAttribute("data-index");
      if (raw === null) continue;
      heightsRef.current.set(Number(raw), child.offsetHeight);
    }

    const parent = scrollParentOf(root);
    const rootRect = root.getBoundingClientRect();
    let viewTop: number;
    let viewHeight: number;
    if (parent) {
      viewTop = parent.getBoundingClientRect().top - rootRect.top;
      viewHeight = parent.clientHeight;
    } else {
      viewTop = -rootRect.top;
      viewHeight = window.innerHeight;
    }
    const viewBottom = viewTop + viewHeight;
    const heightAt = (index: number) => heightsRef.current.get(index) ?? estimate;

    let acc = 0;
    let start = 0;
    let end = itemCount;
    let topSpacer = 0;
    for (let index = 0; index < itemCount; index += 1) {
      const top = acc;
      acc += heightAt(index);
      if (acc < viewTop - overscanPx) {
        start = index + 1;
        topSpacer = acc;
        continue;
      }
      if (top > viewBottom + overscanPx) {
        end = index;
        break;
      }
    }
    let bottomSpacer = 0;
    for (let index = end; index < itemCount; index += 1) {
      bottomSpacer += heightAt(index);
    }

    setRange((previous) =>
      previous.start === start &&
      previous.end === end &&
      previous.topSpacer === topSpacer &&
      previous.bottomSpacer === bottomSpacer
        ? previous
        : { start, end, topSpacer, bottomSpacer },
    );
  }, [itemCount, estimate, overscanPx]);

  // Runs after every commit on purpose (no dep array): newly mounted rows
  // get measured and the window re-seats with real heights. The setRange
  // guard above makes steady state a no-op, so this performs no extra
  // render once heights converge.
  useLayoutEffect(() => {
    refresh();
  });

  // Follow scrolling and viewport resizes; rAF-throttled to one refresh
  // per frame.
  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const parent = scrollParentOf(root);
    let frame = 0;
    const schedule = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        refresh();
      });
    };
    parent?.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    return () => {
      parent?.removeEventListener("scroll", schedule);
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [refresh]);

  return { ...range, rootRef };
}

/** Nearest ancestor that actually scrolls vertically, if any. */
function scrollParentOf(node: HTMLElement): HTMLElement | null {
  let current = node.parentElement;
  while (current) {
    if (/(auto|scroll|overlay)/.test(getComputedStyle(current).overflowY)) return current;
    current = current.parentElement;
  }
  return null;
}
