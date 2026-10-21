import { Component, type ErrorInfo, type ReactNode } from "react";
import { TriangleAlert } from "lucide-react";
import { logUiError } from "../lib/tauri";

export interface ErrorBoundaryCopy {
  title: string;
  message: string;
  retry: string;
  reload: string;
}

interface ErrorBoundaryProps {
  /** Stable area name for the backend log (`app`, `skills`, `mcps`, …). */
  area: string;
  copy: ErrorBoundaryCopy;
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/**
 * Catches render crashes below it so one bad payload cannot white-screen
 * the whole shell. Retry re-renders the children in place; parents that
 * pass `key={view}` additionally reset the boundary on navigation.
 * Crashes are reported to the Rust core for diagnostics.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error(`[ErrorBoundary:${this.props.area}]`, error);
    void logUiError(
      this.props.area,
      `${error.name}: ${error.message}`,
      info.componentStack ?? undefined,
    ).catch(() => undefined);
  }

  private retry = (): void => {
    this.setState({ error: null });
  };

  private reload = (): void => {
    window.location.reload();
  };

  render(): ReactNode {
    const { error } = this.state;
    if (!error) return this.props.children;

    const { copy } = this.props;
    return (
      <div
        role="alert"
        className="flex flex-col items-center gap-2 rounded-xl border border-amber-500/25 bg-amber-500/[0.06] px-4 py-8 text-center"
      >
        <TriangleAlert className="size-5 text-amber-400" />
        <p className="text-sm font-semibold text-zinc-100">{copy.title}</p>
        <p className="max-w-[52ch] text-[11.5px] leading-relaxed text-zinc-400">{copy.message}</p>
        <p className="max-w-full truncate font-mono text-[10px] text-zinc-600">{error.message}</p>
        <div className="mt-1 flex gap-2">
          <button
            type="button"
            onClick={this.retry}
            className="rounded-lg border border-white/10 bg-white/[0.02] px-3 py-1.5 text-[11.5px] font-medium text-zinc-300 hover:text-zinc-100"
          >
            {copy.retry}
          </button>
          <button
            type="button"
            onClick={this.reload}
            className="rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-1.5 text-[11.5px] font-medium text-emerald-200 hover:bg-emerald-500/25"
          >
            {copy.reload}
          </button>
        </div>
      </div>
    );
  }
}
