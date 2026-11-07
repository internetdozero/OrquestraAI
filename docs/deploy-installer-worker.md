# Installer Edge Worker — Deploy Guide

`https://orquestra.internetdozero.com.br/install.sh` and
`https://orquestra.internetdozero.com.br/install.ps1` are served by the
Cloudflare Worker in `workers/installer/worker.js`, never by pasted copies:
every request resolves the latest GitHub release tag and serves that
installer at the tag byte-identical, with a SHA-256 `ETag`,
`If-None-Match` → `304` handling, and bounded caching (5-minute edge TTL
plus instant invalidation when the resolved tag no longer matches the
cached entry).

## Deploy

```sh
wrangler login            # or export CLOUDFLARE_API_TOKEN
sh scripts/deploy-installer-worker.sh
```

The script deploys from `workers/installer/`, then fails closed unless the
served bytes of both `install.sh` and `install.ps1` hash exactly like the
same files at the canonical latest tag (local `git show <tag>:<file>` when
the tag exists, else the raw tag URL). A custom worker base URL can be
passed as the first argument; a trailing `/install.sh` or `/install.ps1` is
accepted and trimmed.

## Custom domain

Bind `orquestra.internetdozero.com.br/*` (or the two `/install.*` paths) to
the `orquestra-installer` worker in the Cloudflare dashboard (or uncomment
the `routes` line in `workers/installer/wrangler.toml`).

## Cache behavior

- Tag lookups are memoized for 5 minutes; installer bytes are edge-cached
  for 5 minutes and keyed with an `x-orquestra-tag` header.
- Publishing a new release converges without manual purging: the next
  lookup resolves the new tag, the cached entry's tag no longer matches,
  and the worker refetches upstream.
- `curl -s https://orquestra.internetdozero.com.br/install.sh | sha256sum`
  and the same for `install.ps1` must equal each file's hash at the tag; the
  deploy script checks both.
