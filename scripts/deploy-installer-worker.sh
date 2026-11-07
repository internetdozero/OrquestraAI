#!/bin/sh
# Deploy the orquestra installer edge worker and verify 1:1 byte parity.
#
# Usage: sh scripts/deploy-installer-worker.sh [worker-url]
#   worker-url defaults to https://orquestra.internetdozero.com.br; a URL
#   ending in /install.sh or /install.ps1 is accepted and trimmed.
#
# Requires: wrangler authenticated (`wrangler login` or CLOUDFLARE_API_TOKEN),
# curl, sha256sum/shasum, git. Exits non-zero unless both `/install.sh` and
# `/install.ps1` are byte-identical to the canonical latest release tag.
set -e

REPO_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
WORKER_BASE="${1:-https://orquestra.internetdozero.com.br}"
WORKER_BASE=$(printf '%s' "$WORKER_BASE" | sed -e 's#/install\.sh$##' -e 's#/install\.ps1$##' -e 's#/$##')
REPO="internetdozero/OrquestraAI"
INSTALLERS="install.sh install.ps1"

if command -v wrangler >/dev/null 2>&1; then
    WRANGLER="wrangler"
elif command -v npx >/dev/null 2>&1; then
    WRANGLER="npx -y wrangler"
else
    echo "❌ Error: wrangler (or npx) is required to deploy." >&2
    exit 1
fi

echo "🚀 Deploying installer worker from $REPO_DIR/workers/installer ..."
(cd "$REPO_DIR/workers/installer" && $WRANGLER deploy)

echo "🔍 Resolving canonical release tag ..."
TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep -o '"tag_name": *"[^"]*"' | head -n 1 | cut -d'"' -f4)
if [ -z "$TAG" ]; then
    echo "❌ Error: could not resolve the latest release tag." >&2
    exit 1
fi
echo "ℹ️  Canonical tag: $TAG"

TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/orquestra-worker-verify.XXXXXXXXXX")
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

if command -v sha256sum >/dev/null 2>&1; then
    HASH_CMD="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
    HASH_CMD="shasum -a 256"
else
    echo "❌ Error: 'sha256sum' or 'shasum' is required for the parity check." >&2
    exit 1
fi

for INSTALLER in $INSTALLERS; do
    if git -C "$REPO_DIR" rev-parse "$TAG^{commit}" >/dev/null 2>&1; then
        git -C "$REPO_DIR" show "$TAG:$INSTALLER" > "$TMP_DIR/canonical"
    else
        curl -fsSL "https://raw.githubusercontent.com/${REPO}/${TAG}/${INSTALLER}" -o "$TMP_DIR/canonical"
    fi
    curl -fsSL "$WORKER_BASE/$INSTALLER" -o "$TMP_DIR/served"

    CANONICAL=$($HASH_CMD "$TMP_DIR/canonical" | cut -d' ' -f1)
    SERVED=$($HASH_CMD "$TMP_DIR/served" | cut -d' ' -f1)
    echo "$INSTALLER canonical ($TAG): $CANONICAL"
    echo "$INSTALLER served   (edge): $SERVED"
    if [ "$CANONICAL" != "$SERVED" ]; then
        echo "❌ Parity check FAILED: edge bytes differ from $INSTALLER at $TAG." >&2
        echo "The edge cache may still be converging (TTL is 5 minutes); retry, then investigate." >&2
        exit 1
    fi
    rm -f "$TMP_DIR/canonical" "$TMP_DIR/served"
done
echo "✓ Parity check passed: edge serves $INSTALLERS at $TAG byte-identical."
