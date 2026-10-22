/**
 * orquestra installer edge worker.
 *
 * Serves `/install.sh` byte-identical to the canonical release tag instead
 * of a stale hardcoded copy: every request resolves the latest GitHub
 * release tag, fetches `install.sh` at that tag from raw.githubusercontent,
 * and answers with a SHA-256 `ETag` plus bounded caching. A new release
 * invalidates within minutes (short edge TTL plus instant invalidation when
 * the resolved tag no longer matches the cached entry's tag).
 *
 * Non-GET/HEAD methods are refused; every other path 404s except `/`
 * (repo redirect) and `/healthz`.
 */

const REPO = "internetdozero/OrquestraAI";
const TAG_API = `https://api.github.com/repos/${REPO}/releases/latest`;
const RAW_AT = (tag) => `https://raw.githubusercontent.com/${REPO}/${tag}/install.sh`;
const UA = "orquestra-installer-worker";
/** How long a resolved tag is trusted before re-querying the API. */
const TAG_TTL_MS = 5 * 60 * 1000;

let tagCache = { tag: null, at: 0 };

async function latestTag() {
  const now = Date.now();
  if (tagCache.tag && now - tagCache.at < TAG_TTL_MS) {
    return tagCache.tag;
  }
  const res = await fetch(TAG_API, {
    headers: { "user-agent": UA, accept: "application/vnd.github+json" },
  });
  if (!res.ok) {
    if (tagCache.tag) return tagCache.tag;
    throw new Error(`release tag lookup failed: HTTP ${res.status}`);
  }
  const { tag_name: tag } = await res.json();
  if (typeof tag !== "string" || tag.length === 0) {
    if (tagCache.tag) return tagCache.tag;
    throw new Error("release tag lookup returned no tag");
  }
  tagCache = { tag, at: now };
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

async function serveInstaller(request, ctx) {
  const tag = await latestTag();
  const cache = caches.default;
  const cached = request.method === "GET" ? await cache.match(request) : null;

  if (cached && cached.headers.get("x-orquestra-tag") === tag) {
    const etag = cached.headers.get("etag");
    if (etagMatches(request, etag)) {
      return new Response(null, {
        status: 304,
        headers: { etag, "x-orquestra-tag": tag },
      });
    }
    return new Response(request.method === "HEAD" ? null : await cached.arrayBuffer(), {
      status: 200,
      headers: cached.headers,
    });
  }

  const upstream = await fetch(RAW_AT(tag), { headers: { "user-agent": UA } });
  if (!upstream.ok) {
    throw new Error(`upstream install.sh at ${tag} failed: HTTP ${upstream.status}`);
  }
  const body = await upstream.arrayBuffer();
  const etag = `"${sha256Hex(await crypto.subtle.digest("SHA-256", body))}"`;
  const headers = new Headers({
    "content-type": "text/x-shellscript; charset=utf-8",
    etag,
    "cache-control": "public, max-age=300",
    "x-orquestra-tag": tag,
  });

  if (etagMatches(request, etag)) {
    return new Response(null, { status: 304, headers });
  }
  const response = new Response(request.method === "HEAD" ? null : body, {
    status: 200,
    headers,
  });
  if (request.method === "GET") {
    ctx.waitUntil(cache.put(request, new Response(body.slice(0), { status: 200, headers })));
  }
  return response;
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
    if (url.pathname !== "/install.sh") {
      return new Response("not found", { status: 404 });
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("method not allowed", { status: 405 });
    }
    try {
      return await serveInstaller(request, ctx ?? { waitUntil: () => {} });
    } catch (error) {
      return new Response(`installer unavailable: ${error.message}`, {
        status: 502,
        headers: { "content-type": "text/plain; charset=utf-8" },
      });
    }
  },
};
