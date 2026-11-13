/**
 * Loads the developer fixture graph on demand.
 *
 * `fixtures.ts` is imported by every view and must stay tiny; the dataset
 * lives in `devFixtures.ts` and is only reachable through this dynamic
 * import. Vite emits it as a separate chunk, so the production bundle never
 * contains the mocks or the absolute developer paths inside them.
 */
export function loadDevFixtures(): Promise<typeof import("./devFixtures.ts")> {
  // The literal `import.meta.env.DEV` is replaced with `false` by the bundler,
  // which makes this branch statically unreachable in a production build and
  // keeps the fixture chunk out of `dist/` entirely.
  if (!import.meta.env.DEV) {
    return Promise.reject(new Error("developer fixtures are unavailable in a production build"));
  }
  return import("./devFixtures.ts");
}
