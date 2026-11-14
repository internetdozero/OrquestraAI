/**
 * ownhub installer edge worker.
 *
 * Serves `/install.sh` and `/install.ps1` byte-identical to the canonical
 * release tag instead of a stale hardcoded copy: every request resolves the
 * latest GitHub release tag, fetches that installer at the tag from
 * raw.githubusercontent, and answers with a SHA-256 `ETag` plus bounded
 * caching. A new release invalidates within minutes (short edge TTL plus
 * instant invalidation when the resolved tag no longer matches the cached
 * entry's tag).
 *
 * Non-GET/HEAD methods are refused; every other path 404s except `/`
 * (repo redirect) and `/healthz`.
 */

const REPO = "OwnAnAI/ownhub";
const LATEST_RELEASE = `https://github.com/${REPO}/releases/latest`;
const RAW_AT = (tag, asset) => `https://raw.githubusercontent.com/${REPO}/${tag}/${asset}`;
const UA = "ownhub-installer-worker";

/** The installers this worker mirrors, keyed by request path. */
const INSTALLERS = new Map([
  ["/install.sh", { asset: "install.sh", contentType: "text/x-shellscript; charset=utf-8" }],
  [
    "/install.ps1",
    // Plain text keeps a browser from downloading it as a file and keeps
    // `irm | iex` (which only reads the body) working either way.
    { asset: "install.ps1", contentType: "text/plain; charset=utf-8" },
  ],
]);
/** How long a resolved tag is trusted before re-querying GitHub. */
const TAG_TTL_MS = 5 * 60 * 1000;
/** How long the last good installer copy survives as an outage fallback. */
const LAST_GOOD_TTL_S = 30 * 24 * 60 * 60;
/** GitHub answers are retried this many times on 5xx/429/network errors. */
const FETCH_ATTEMPTS = 3;
const RETRY_DELAY_MS = 150;

// Synthetic Cache API keys (never fetched): shared by every isolate in a
// colo, so a cold isolate does not have to reach GitHub on its first request.
const INTERNAL = "https://ownhub-installer.internal";
const TAG_KEY = `${INTERNAL}/latest-tag`;
const LAST_GOOD_KEY = (asset) => `${INTERNAL}/last-good/${asset}`;

let tagCache = { tag: null, at: 0 };

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * GitHub intermittently answers Worker subrequests with 5xx (observed:
 * `releases/latest` -> HTTP 504), so transient failures are retried before
 * the caller falls back to the last good copy.
 */
async function fetchWithRetry(url, init) {
  let lastError;
  for (let attempt = 1; attempt <= FETCH_ATTEMPTS; attempt += 1) {
    try {
      const res = await fetch(url, init);
      if (res.status < 500 && res.status !== 429) return res;
      lastError = new Error(`HTTP ${res.status}`);
      lastError.status = res.status;
      if (attempt === FETCH_ATTEMPTS) return res;
    } catch (error) {
      lastError = error;
      if (attempt === FETCH_ATTEMPTS) throw error;
    }
    await sleep(RETRY_DELAY_MS * attempt);
  }
  throw lastError;
}

async function cachedTag(cache) {
  const hit = await cache.match(TAG_KEY);
  if (!hit) return null;
  const at = Number(hit.headers.get("x-ownhub-at"));
  const tag = hit.headers.get("x-ownhub-tag");
  if (!tag || !Number.isFinite(at) || Date.now() - at >= TAG_TTL_MS) return null;
  return { tag, at };
}

async function latestTag(cache, ctx) {
  const now = Date.now();
  if (tagCache.tag && now - tagCache.at < TAG_TTL_MS) {
    return tagCache.tag;
  }
  const shared = await cachedTag(cache);
  if (shared) {
    tagCache = shared;
    return shared.tag;
  }
  const res = await fetchWithRetry(LATEST_RELEASE, {
    redirect: "manual",
    headers: { "user-agent": UA },
  });
  const location = res.headers.get("location");
  if (res.status < 300 || res.status >= 400 || !location) {
    if (tagCache.tag) return tagCache.tag;
    throw new Error(`release tag lookup failed: HTTP ${res.status}`);
  }
  const releaseUrl = new URL(location, LATEST_RELEASE);
  const match = releaseUrl.pathname.match(/\/releases\/tag\/([^/]+)$/);
  const tag = match ? decodeURIComponent(match[1]) : null;
  if (!tag) {
    if (tagCache.tag) return tagCache.tag;
    throw new Error("release tag redirect did not contain a release tag");
  }
  tagCache = { tag, at: now };
  ctx.waitUntil(
    cache.put(
      TAG_KEY,
      new Response(null, {
        headers: {
          "cache-control": `public, max-age=${TAG_TTL_MS / 1000}`,
          "x-ownhub-tag": tag,
          "x-ownhub-at": String(now),
        },
      }),
    ),
  );
  return tag;
}

function sha256Hex(digest) {
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function etagMatches(request, etag) {
  const inm = request.headers.get("if-none-match");
  if (!inm || !etag) return false;
  return inm
    .split(",")
    .map((s) => s.trim())
    .includes(etag);
}

function respondWith(request, body, headers) {
  const etag = headers.get("etag");
  if (etagMatches(request, etag)) {
    return new Response(null, {
      status: 304,
      headers: { etag, "x-ownhub-tag": headers.get("x-ownhub-tag") ?? "" },
    });
  }
  return new Response(request.method === "HEAD" ? null : body, { status: 200, headers });
}

/**
 * Serves the last installer this colo fetched successfully when GitHub is
 * unreachable, instead of failing the `curl | sh` with a 502. Returns null
 * when no copy exists yet (then the original error surfaces).
 */
async function serveLastGood(request, cache, installer) {
  const stored = await cache.match(LAST_GOOD_KEY(installer.asset));
  if (!stored) return null;
  const headers = new Headers(stored.headers);
  headers.set("cache-control", "public, max-age=60");
  headers.set("x-ownhub-fallback", "last-good");
  return respondWith(request, await stored.arrayBuffer(), headers);
}

async function serveFresh(request, ctx, installer, cache) {
  const tag = await latestTag(cache, ctx);
  const cacheKey = new Request(request.url, { method: "GET" });
  const cached = await cache.match(cacheKey);

  if (cached && cached.headers.get("x-ownhub-tag") === tag) {
    return respondWith(request, await cached.arrayBuffer(), new Headers(cached.headers));
  }

  const upstream = await fetchWithRetry(RAW_AT(tag, installer.asset), {
    headers: { "user-agent": UA },
  });
  if (!upstream.ok) {
    throw new Error(`upstream ${installer.asset} at ${tag} failed: HTTP ${upstream.status}`);
  }
  const body = await upstream.arrayBuffer();
  const etag = `"${sha256Hex(await crypto.subtle.digest("SHA-256", body))}"`;
  const headers = new Headers({
    "content-type": installer.contentType,
    etag,
    "cache-control": "public, max-age=300",
    "x-ownhub-tag": tag,
  });
  const lastGoodHeaders = new Headers(headers);
  lastGoodHeaders.set("cache-control", `public, max-age=${LAST_GOOD_TTL_S}`);
  ctx.waitUntil(
    Promise.all([
      cache.put(cacheKey, new Response(body.slice(0), { status: 200, headers })),
      cache.put(
        LAST_GOOD_KEY(installer.asset),
        new Response(body.slice(0), { status: 200, headers: lastGoodHeaders }),
      ),
    ]),
  );
  return respondWith(request, body, headers);
}

async function serveInstaller(request, ctx, installer) {
  const cache = caches.default;
  try {
    return await serveFresh(request, ctx, installer, cache);
  } catch (error) {
    const fallback = await serveLastGood(request, cache, installer).catch(() => null);
    if (fallback) return fallback;
    throw error;
  }
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (url.pathname === "/") {
      return Response.redirect(`https://github.com/${REPO}`, 302);
    }
    if (url.pathname === "/healthz") {
      return new Response("ok", { headers: { "content-type": "text/plain; charset=utf-8" } });
    }
    const installer = INSTALLERS.get(url.pathname);
    if (!installer) {
      return new Response("not found", { status: 404 });
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("method not allowed", { status: 405 });
    }
    try {
      return await serveInstaller(request, ctx ?? { waitUntil: () => {} }, installer);
    } catch (error) {
      return new Response(`installer unavailable: ${error.message}`, {
        status: 502,
        headers: { "content-type": "text/plain; charset=utf-8" },
      });
    }
  },
};
