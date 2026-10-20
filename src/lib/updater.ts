/** Minimal GitHub releases lookup: fetch-only, no dependencies. */

export interface ReleaseCheckResult {
  isLatest: boolean;
  latestVersion: string;
  releaseUrl?: string;
  releaseNotes?: string;
  publishedAt?: string;
}

const LATEST_URL =
  "https://api.github.com/repos/internetdozero/OrquestraAI/releases/latest";

/** `v0.1.0` → `0.1.0` so tags compare against the bare app version. */
function cleanTag(tag: string): string {
  return tag.trim().replace(/^[vV]/, "");
}

interface ParsedVersion {
  numbers: number[];
  prerelease: string[];
}

/**
 * Semver core plus pre-release. `0.1.0-alpha.3` must sort before `0.1.0`.
 * Splitting on `.` alone turns the pre-release into extra numeric segments
 * (`0-alpha` → 0, then `3`), so the alpha build looks newer than the release.
 */
function parseVersion(value: string): ParsedVersion {
  const cleaned = cleanTag(value).split("+")[0] ?? "";
  const dash = cleaned.indexOf("-");
  const core = dash === -1 ? cleaned : cleaned.slice(0, dash);
  const pre = dash === -1 ? "" : cleaned.slice(dash + 1);
  const numbers = (core.length === 0 ? [] : core.split(".")).map((part) =>
    /^\d+$/.test(part) ? Number(part) : 0,
  );
  const prerelease =
    pre.length === 0 ? [] : pre.split(".").filter((part) => part.length > 0);
  return { numbers, prerelease };
}

function compareIdentifiers(left: string, right: string): number {
  const leftNum = /^\d+$/.test(left);
  const rightNum = /^\d+$/.test(right);
  if (leftNum && rightNum) {
    const diff = Number(left) - Number(right);
    return diff === 0 ? 0 : diff < 0 ? -1 : 1;
  }
  if (leftNum !== rightNum) return leftNum ? -1 : 1;
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

/** -1 when `left` is behind `right`, 0 equal, 1 ahead. Pre-releases lose to their release. */
function compareVersions(left: string, right: string): number {
  const a = parseVersion(left);
  const b = parseVersion(right);
  const width = Math.max(a.numbers.length, b.numbers.length);
  for (let index = 0; index < width; index += 1) {
    const av = a.numbers[index] ?? 0;
    const bv = b.numbers[index] ?? 0;
    if (av !== bv) return av < bv ? -1 : 1;
  }
  if (a.prerelease.length === 0 && b.prerelease.length === 0) return 0;
  if (a.prerelease.length === 0) return 1;
  if (b.prerelease.length === 0) return -1;
  const preWidth = Math.max(a.prerelease.length, b.prerelease.length);
  for (let index = 0; index < preWidth; index += 1) {
    const av = a.prerelease[index];
    const bv = b.prerelease[index];
    if (av === undefined) return -1;
    if (bv === undefined) return 1;
    const diff = compareIdentifiers(av, bv);
    if (diff !== 0) return diff;
  }
  return 0;
}

/**
 * Latest published release versus the running version. A 404 (no public
 * release yet) or any network failure resolves as up-to-date with the
 * current version — the check must never crash the settings UI.
 */
export async function checkLatestRelease(
  currentVersion: string,
): Promise<ReleaseCheckResult> {
  try {
    const response = await fetch(LATEST_URL, {
      headers: {
        Accept: "application/vnd.github.v3+json",
        "User-Agent": "OrquestraAI-App",
      },
    });
    if (!response.ok) return { isLatest: true, latestVersion: currentVersion };

    const data = (await response.json()) as {
      tag_name?: string;
      html_url?: string;
      body?: string;
      published_at?: string;
    };
    if (!data.tag_name) return { isLatest: true, latestVersion: currentVersion };

    const latestVersion = cleanTag(data.tag_name);
    if (compareVersions(latestVersion, cleanTag(currentVersion)) <= 0) {
      return { isLatest: true, latestVersion: currentVersion };
    }
    return {
      isLatest: false,
      latestVersion,
      releaseUrl: data.html_url,
      releaseNotes: data.body,
      publishedAt: data.published_at,
    };
  } catch {
    return { isLatest: true, latestVersion: currentVersion };
  }
}
