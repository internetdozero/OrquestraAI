import { Fragment, useEffect, useRef, type ReactNode } from "react";
import { useVirtualRows } from "../hooks/useVirtualRows";

interface VirtualCardGridProps<T> {
  items: readonly T[];
  keyOf: (item: T) => string;
  renderItem: (item: T) => ReactNode;
  /** Grid classes, e.g. `grid grid-cols-1 gap-3 md:grid-cols-2`. */
  className: string;
  /** Card to reveal when it changes, e.g. one that just changed state. */
  focusKey?: string | null;
  /** Row height assumed before a row has been measured. */
  estimate?: number;
}

/**
 * Card grid that renders only the rows near the viewport once the list is
 * long enough to matter (`VIRTUALIZE_AFTER`). Short lists render exactly as
 * before — no spacers, no listeners — so the common vault pays nothing.
 *
 * `focusKey` keeps the grid's job of showing a card that just moved: if that
 * card is outside the window it scrolls to the card's row estimate, then
 * reveals the card exactly once it mounts.
 */
export function VirtualCardGrid<T>({
  items,
  keyOf,
  renderItem,
  className,
  focusKey = null,
  estimate,
}: VirtualCardGridProps<T>) {
  const { gridRef, columns, virtual, rows, scrollToRow } = useVirtualRows({
    itemCount: items.length,
    estimate,
  });
  const revealed = useRef<string | null>(null);

  useEffect(() => {
    if (!focusKey) {
      revealed.current = null;
      return;
    }
    if (revealed.current === focusKey) return;

    const grid = gridRef.current;
    if (!grid) return;

    const card = grid.querySelector<HTMLElement>(
      `[data-card-id="${CSS.escape(focusKey)}"]`,
    );
    if (card) {
      card.scrollIntoView({ block: "nearest", behavior: "smooth" });
      revealed.current = focusKey;
      return;
    }

    if (!virtual) return;

    const index = items.findIndex((item) => keyOf(item) === focusKey);
    if (index >= 0) scrollToRow(Math.floor(index / columns));
  }, [
    focusKey,
    items,
    keyOf,
    columns,
    gridRef,
    scrollToRow,
    virtual,
    // Re-runs when the window moves, so the card is revealed once it mounts.
    rows.start,
  ]);

  const visible = items.slice(rows.start * columns, rows.end * columns);

  return (
    <div ref={gridRef} className={className}>
      {rows.topSpace > 0 && <Spacer slot="top" height={rows.topSpace} />}

      {visible.map((item) => (
        <Fragment key={keyOf(item)}>{renderItem(item)}</Fragment>
      ))}

      {rows.bottomSpace > 0 && (
        <Spacer slot="bottom" height={rows.bottomSpace} />
      )}
    </div>
  );
}

/** Full-width gap standing in for the rows outside the window. */
function Spacer({
  slot,
  height,
}: {
  slot: "top" | "bottom";
  height: number;
}) {
  return (
    <div
      aria-hidden="true"
      data-virtual-slot={slot}
      style={{ height, gridColumn: "1 / -1" }}
    />
  );
}
