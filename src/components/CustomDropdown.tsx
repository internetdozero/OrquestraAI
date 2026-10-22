import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, type LucideIcon } from "lucide-react";
import { cn } from "../lib/utils";

export interface DropdownOption<T extends string> {
  id: T;
  label: string;
}

interface CustomDropdownProps<T extends string> {
  value: T;
  options: readonly DropdownOption<T>[];
  onChange: (id: T) => void;
  ariaLabel: string;
  icon?: LucideIcon;
  title?: string;
  className?: string;
}

/** Minimum menu width, matching the old `min-w-[13.5rem]`. */
const MENU_MIN_WIDTH = 216;
/** Breathing room kept between the menu and the viewport edges. */
const VIEWPORT_MARGIN = 8;
/** Space between the trigger and the menu. */
const TRIGGER_GAP = 6;

interface MenuBox {
  top: number;
  left: number;
  minWidth: number;
  maxHeight: number;
  /** Which edge the enter animation grows from. */
  placement: "bottom" | "top";
}

/**
 * Position the menu inside the viewport instead of inside its offset parent.
 * Right-aligned by default, mirrored to left-aligned when the right edge would
 * leave the screen, and flipped above the trigger when there is no room below.
 */
function measureMenu(trigger: DOMRect, menu: HTMLElement | null): MenuBox {
  const viewportW = window.innerWidth;
  const viewportH = window.innerHeight;
  const width = Math.max(MENU_MIN_WIDTH, menu?.offsetWidth ?? MENU_MIN_WIDTH);
  const height = menu?.offsetHeight ?? 0;

  const maxLeft = Math.max(VIEWPORT_MARGIN, viewportW - width - VIEWPORT_MARGIN);
  // Prefer the trigger's right edge; fall back to its left edge on compact
  // layouts where a wrapped toolbar pushes the control toward the viewport.
  let left = trigger.right - width;
  if (left < VIEWPORT_MARGIN) left = trigger.left;
  left = Math.min(Math.max(left, VIEWPORT_MARGIN), maxLeft);

  const below = trigger.bottom + TRIGGER_GAP;
  const above = trigger.top - TRIGGER_GAP - height;
  const flipAbove =
    height > 0 && below + height > viewportH - VIEWPORT_MARGIN && above >= VIEWPORT_MARGIN;
  // A safety clamp: if the trigger itself is partly off-screen (short window,
  // wrapped toolbar), keep the menu fully inside the viewport anyway.
  const maxTop = Math.max(VIEWPORT_MARGIN, viewportH - height - VIEWPORT_MARGIN);
  const top = Math.min(Math.max(flipAbove ? above : below, VIEWPORT_MARGIN), maxTop);

  return {
    top: Math.round(top),
    left: Math.round(left),
    minWidth: Math.round(Math.min(width, viewportW - VIEWPORT_MARGIN * 2)),
    maxHeight: Math.round(Math.max(160, viewportH - VIEWPORT_MARGIN * 2)),
    placement: flipAbove ? "top" : "bottom",
  };
}

/**
 * Dark listbox used in place of the OS-native `<select>` popover.
 *
 * The menu is portaled to `document.body` and positioned with `position:
 * fixed`, so a scrollable or `overflow-hidden` ancestor can no longer clip it
 * (or swallow the click that opened it). It mounts already placed, and a
 * one-shot enter animation avoids the opacity transition that used to fade the
 * panel back out on the opening click.
 */
export function CustomDropdown<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  icon: Icon,
  title,
  className,
}: CustomDropdownProps<T>) {
  const [open, setOpen] = useState(false);
  const [box, setBox] = useState<MenuBox | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLUListElement>(null);
  const listId = useId();
  const current = options.find((option) => option.id === value) ?? options[0];

  const place = useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return;

    const next = measureMenu(trigger.getBoundingClientRect(), menuRef.current);
    setBox((previous) =>
      previous &&
      previous.top === next.top &&
      previous.left === next.left &&
      previous.minWidth === next.minWidth &&
      previous.maxHeight === next.maxHeight &&
      previous.placement === next.placement
        ? previous
        : next,
    );
  }, []);

  // The portal is committed with `open`, so its height is measurable here and
  // the menu is shown in its final place on the first paint.
  useLayoutEffect(() => {
    if (!open) {
      setBox(null);
      return;
    }

    place();
  }, [open, place]);

  // CSP-safe positioning: style-src carries no 'unsafe-inline', so the
  // measured box goes through CSSOM. A layout effect still places the menu
  // before the first paint, as the style prop did before.
  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) return;
    if (!box) {
      menu.style.visibility = "hidden";
      return;
    }
    menu.style.top = `${box.top}px`;
    menu.style.left = `${box.left}px`;
    menu.style.minWidth = `${box.minWidth}px`;
    menu.style.maxHeight = `${box.maxHeight}px`;
    menu.style.visibility = "visible";
    menu.style.transformOrigin = box.placement === "top" ? "bottom center" : "top center";
  }, [box, open]);

  useEffect(() => {
    if (!open) return;

    // Scrolling is coalesced to one layout read per frame, and the read stays
    // out of the scroll handler itself so it never blocks the main thread.
    let frame = 0;
    const onScroll = () => {
      if (frame) return;

      frame = window.requestAnimationFrame(() => {
        frame = 0;
        place();
      });
    };

    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (rootRef.current?.contains(target) || menuRef.current?.contains(target)) return;

      setOpen(false);
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;

      setOpen(false);
      triggerRef.current?.focus();
    };

    // Capture phase plus `passive` so the reposition never blocks the scroll
    // and still follows the trigger inside the main scroll pane.
    document.addEventListener("pointerdown", onPointerDown, true);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("scroll", onScroll, {
      capture: true,
      passive: true,
    });
    window.addEventListener("resize", place);

    return () => {
      if (frame) window.cancelAnimationFrame(frame);
      document.removeEventListener("pointerdown", onPointerDown, true);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", place);
    };
  }, [open, place]);

  return (
    <div ref={rootRef} className={cn("relative shrink-0", className)}>
      <button
        ref={triggerRef}
        type="button"
        title={title}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={ariaLabel}
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => setOpen((value) => !value)}
        className="flex min-w-0 max-w-full items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.03] py-1.5 pr-2 pl-2 text-[11px] font-medium whitespace-nowrap text-zinc-300 transition hover:border-white/20 focus-visible:border-emerald-500/40 focus-visible:ring-2 focus-visible:ring-emerald-500/20 focus-visible:outline-none"
      >
        {Icon && <Icon className="size-3 shrink-0 text-zinc-500" />}
        <span className="min-w-0 flex-1 truncate text-left">{current?.label}</span>
        <ChevronDown
          className={cn(
            "size-3 shrink-0 text-zinc-500 transition-transform duration-150",
            open && "rotate-180",
          )}
        />
      </button>

      {open &&
        createPortal(
          <ul
            ref={menuRef}
            id={listId}
            role="listbox"
            aria-label={ariaLabel}
            className="dropdown-pop fixed z-40 overflow-y-auto overscroll-contain rounded-xl border border-white/[0.08] bg-zinc-900/95 p-1 shadow-2xl backdrop-blur-md"
          >
            {options.map((option) => {
              const active = option.id === value;
              return (
                <li key={option.id} role="option" aria-selected={active}>
                  <button
                    type="button"
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => {
                      onChange(option.id);
                      setOpen(false);
                    }}
                    className={cn(
                      "flex w-full items-center justify-between gap-3 rounded-lg px-2.5 py-1.5 text-left text-xs text-zinc-300 transition-colors hover:bg-emerald-500/10 hover:text-emerald-300",
                      active && "bg-emerald-500/10 text-emerald-300",
                    )}
                  >
                    <span>{option.label}</span>
                    {active && <Check className="size-3 shrink-0" aria-hidden="true" />}
                  </button>
                </li>
              );
            })}
          </ul>,
          document.body,
        )}
    </div>
  );
}
