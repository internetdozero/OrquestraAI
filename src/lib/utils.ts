import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { KeyboardEvent } from "react";

/**
 * Merge conditional class names and de-duplicate conflicting Tailwind utilities.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Native-form-like implicit submission for dialogs built from divs: Enter
 * inside a single-line field confirms. Multi-line fields, toggles, sliders
 * and elements opting out via `data-no-enter` never submit, and a nested
 * handler that already consumed the key always wins via defaultPrevented.
 */
export function confirmOnEnter(event: KeyboardEvent, confirm: () => void): void {
  if (event.key !== "Enter" || event.defaultPrevented) return;
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  if (event.nativeEvent.isComposing) return;
  const target = event.target as HTMLElement | null;
  if (!(target instanceof HTMLInputElement)) return;
  if (target.dataset.noEnter !== undefined) return;
  switch (target.type) {
    case "text":
    case "search":
    case "url":
    case "tel":
    case "number":
    case "password":
      break;
    default:
      return;
  }
  event.preventDefault();
  confirm();
}

/** Format a raw token estimate as a compact human-readable string. */
export function formatTokens(tokens: number): string {
  if (tokens >= 1_000) {
    return `${(tokens / 1_000).toFixed(1)}k`;
  }
  return `${tokens}`;
}
