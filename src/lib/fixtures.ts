/**
 * Dev-only switches.
 *
 * This module must stay dependency-free: every view imports these helpers,
 * including in production, and a static import of the fixture data would drag
 * the whole mock dataset (and the developer paths inside it) into the shipped
 * bundle. The fixtures themselves live in `devFixtures.ts` and are reached
 * through a dynamic `import()` from the dev branches only.
 */

const STORAGE_KEY = "orquestra.fixtureMode";

/**
 * Developer fixture mode: makes the Sessions view and the transcript drawer
 * work in a plain Vite browser preview, with one mock conversation per
 * harness. Never active in a production build.
 *
 * Enable with `?fixtures=1` (remembered in localStorage) and disable with
 * `?fixtures=0`.
 */
export function fixturesEnabled(): boolean {
  if (!import.meta.env.DEV || typeof window === "undefined") return false;
  // Never shadow real session data inside the desktop app, even in dev.
  if ("__TAURI_INTERNALS__" in window) return false;
  const flag = new URLSearchParams(window.location.search).get("fixtures");
  try {
    if (flag === "1") {
      window.localStorage.setItem(STORAGE_KEY, "1");
      return true;
    }
    if (flag === "0") {
      window.localStorage.removeItem(STORAGE_KEY);
      return false;
    }
    return window.localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return flag === "1";
  }
}

/**
 * Browser mock mode: automatic in Vite dev outside the desktop shell, so
 * agents and developers can click through every view in a plain browser.
 * Unlike `fixturesEnabled` (opt-in per view via `?fixtures=1`), this needs
 * no flag. Never true in production builds or inside Tauri, so mocks can
 * never shadow real data.
 */
export function devMockMode(): boolean {
  if (!import.meta.env.DEV || typeof window === "undefined") return false;
  return !("__TAURI_INTERNALS__" in window);
}
