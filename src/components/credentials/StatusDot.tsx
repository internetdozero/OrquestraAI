import { cn } from "../../lib/utils.ts";

/** Solid dot tint per health state. Status colours must not follow the accent
 * theme (index.css remaps `emerald-*` to the operator's accent), so healthy
 * uses the untouched `green-*` ramp and the dot stays a traffic light. */
export function healthDotClass(health: string): string {
  switch (health) {
    case "Healthy":
    case "healthy":
      return "bg-green-500";
    case "Degraded":
      return "bg-amber-500";
    case "RateLimited":
    case "rateLimited":
      return "bg-orange-500";
    case "QuotaExceeded":
    case "quotaExceeded":
    case "InvalidKey":
    case "invalidKey":
      return "bg-red-500";
    default:
      return "bg-zinc-500";
  }
}

interface StatusDotProps {
  /** Service or key health, in either the Pascal or camelCase spelling. */
  health: string;
  /** Translated health name, used for the tooltip and screen readers. */
  label: string;
  /** `md` fits service rows and headers, `sm` the denser key rows. */
  size?: "sm" | "md";
  className?: string;
}

/** Circular status indicator: no border, no text, one solid colour. */
export function StatusDot({ health, label, size = "md", className }: StatusDotProps) {
  return (
    <span
      role="img"
      title={label}
      aria-label={label}
      className={cn(
        "inline-block shrink-0 rounded-full",
        size === "sm" ? "size-1.5" : "size-2",
        healthDotClass(health),
        className,
      )}
    />
  );
}
