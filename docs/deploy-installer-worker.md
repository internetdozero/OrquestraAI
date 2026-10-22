# Installer Edge Worker — Deploy Guide

`https://orquestra.internetdozero.com.br/install.sh` is served by the
Cloudflare Worker in `workers/installer/worker.js`, never by a pasted copy:
every request resolves the latest GitHub release tag and serves
`install.sh` at that tag byte-identical, with a SHA-256 `ETag`,
`If-None-Match` → `304` handling, and bounded caching (5-minute edge TTL
plus instant invalidation when the resolved tag no longer matches the
cached entry).

## Deploy

```sh
wrangler login            # or export CLOUDFLARE_API_TOKEN
sh scripts/deploy-installer-worker.sh
```

The script deploys from `workers/installer/`, then fails closed unless the
served bytes hash exactly like `install.sh` at the canonical latest tag
(local `git show <tag>:install.sh` when the tag exists, else the raw tag
URL). A custom worker URL can be passed as the first argument.

## Custom domain

Bind `orquestra.internetdozero.com.br/install.sh` to the
`orquestra-installer` worker in the Cloudflare dashboard (or uncomment the
`routes` line in `workers/installer/wrangler.toml`).

## Cache behavior

- Tag lookups are memoized for 5 minutes; installer bytes are edge-cached
  for 5 minutes and keyed with an `x-orquestra-tag` header.
- Publishing a new release converges without manual purging: the next
  lookup resolves the new tag, the cached entry's tag no longer matches,
  and the worker refetches upstream.
- `curl -s https://orquestra.internetdozero.com.br/install.sh | sha256sum`
  must equal the tag's `install.sh` hash; the deploy script checks this.
