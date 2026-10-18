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

/** Numeric dot-segment compare: -1 behind, 0 equal, 1 ahead. */
function compareVersions(left: string, right: string): number {
  const parse = (value: string) =>
    value.split(".").map((part) => Number.parseInt(part, 10) || 0);
  const a = parse(left);
  const b = parse(right);
  for (let index = 0; index < Math.max(a.length, b.length); index += 1) {
    const diff = (a[index] ?? 0) - (b[index] ?? 0);
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
