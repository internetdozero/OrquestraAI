#!/usr/bin/env bash
set -euo pipefail

# OrquestraAI Official Release Build Script
echo "========================================================"
echo "🚀 Building OrquestraAI Official Release..."
echo "========================================================"

# 1. Build Tauri release binary with custom-protocol embedded.
# `tauri build` already runs tauri.conf.json's `beforeBuildCommand`
# (`npm run build`), so the frontend must not be built here as well.
echo "🦀 Compiling Tauri release binary (frontend included)..."
npx tauri build --no-bundle

# 2. Verify output.
# This repo is a Cargo workspace, so artifacts go to the workspace target
# directory (the repo root `target/`), not `src-tauri/target/`. Ask Cargo
# instead of hardcoding a path: a leftover `src-tauri/target/release` would
# otherwise let this script "verify" a stale binary from an older build.
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
RELEASE_BIN="$TARGET_DIR/release/orquestra-ai"

if [ ! -x "$RELEASE_BIN" ]; then
    echo "❌ Error: Release binary not found at $RELEASE_BIN"
    exit 1
fi

echo "========================================================"
echo "✅ Release built successfully at: $RELEASE_BIN"
echo "🔍 Verifying CLI status..."
"$RELEASE_BIN" --mcp-status > /dev/null
"$RELEASE_BIN" --bridge-status > /dev/null
echo "🛡️ Bridge and MCP integrity verified!"
echo "========================================================"
