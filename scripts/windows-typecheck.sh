#!/usr/bin/env bash
# Cross-checks Windows-only Rust from a non-Windows host.
#
# Two steps, because the code splits in two:
#
#   1. The portable crates (`ownhub-core`, `ownhub-bridge`) cross-compile
#      for real, so they are checked and linted for the Windows target.
#   2. The desktop shell (`ownhub`) cannot: `ring`'s build script and
#      `rusqlite`'s bundled SQLite need a Windows C toolchain. Its
#      `#[cfg(windows)]` modules are therefore compiled through the staging
#      crate in `tools/windows-typecheck`, which `#[path]`-includes the real
#      files instead of copying them.
#
# Used by the `windows-cross-check` CI job, and runnable locally before a push
# that touches anything under `cfg(windows)`.
set -euo pipefail

TARGET="${TARGET:-x86_64-pc-windows-msvc}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "error: rustup target $TARGET is not installed." >&2
  echo "       run: rustup target add $TARGET" >&2
  exit 1
fi

# `--all-targets` compiles the test code as well, but Clippy stays on the
# library target: the repository's lint gate is `cargo clippy --workspace`,
# and older test helpers do not satisfy today's pedantic lints.
echo "== portable crates, cross-compiled for $TARGET"
cargo check -p ownhub-core -p ownhub-bridge --all-targets --target "$TARGET"
cargo clippy -p ownhub-core -p ownhub-bridge --target "$TARGET" -- -D warnings

echo "== shimmed shell modules, cross-compiled for $TARGET"
# `-D warnings` so the staging crate cannot go stale by accumulating unused
# shims; the whole graph is reachable through `build_graph()`.
(
  cd tools/windows-typecheck
  cargo fmt --check
  cargo clippy --target "$TARGET" -- -D warnings
)

echo "windows cross-check passed"
