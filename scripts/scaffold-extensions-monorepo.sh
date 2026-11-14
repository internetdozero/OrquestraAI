#!/bin/sh
# Scaffold the `ownhub-extensions` monorepo: unified home for the official
# skills (`skills/<id>/SKILL.md`) and MCP servers (`mcps/<id>/pyproject.toml`).
#
# Usage:
#   ./scripts/scaffold-extensions-monorepo.sh [DEST]
#   ./scripts/scaffold-extensions-monorepo.sh --with-subtrees DEST  # pull existing repos
#
# DEST defaults to ./ownhub-extensions. Never touches /tmp; all work
# happens inside DEST. Idempotent: rerunning keeps existing SKILL.md files.
set -eu

WITH_SUBTREES=0
if [ "${1:-}" = "--with-subtrees" ]; then
  WITH_SUBTREES=1
  shift
fi
DEST="${1:-ownhub-extensions}"

SKILLS="ownhub ownhub-modular-start ownhub-agent-orchestrator ownhub-smart-build-router"
MCPS="ownhub-idea-vault ownhub-session-relay ownhub-janitor ownhub-core ownhub-cloudflare"
ORG="https://github.com/internetdozero"

mkdir -p "$DEST/skills" "$DEST/mcps"
cd "$DEST"

if [ ! -f README.md ]; then
  cat > README.md <<'EOF'
# ownhub-extensions

Unified monorepo for the official OwnHub skills and MCP servers.

- `skills/<skill-id>/SKILL.md` — one folder per skill, importable via
  `ownhub-extensions.git#skills/<skill-id>`.
- `mcps/<mcp-id>/` — one Python package per MCP server (`pyproject.toml`).

Legacy standalone repos remain as pull-through fallbacks until every
consumer tracks this monorepo.
EOF
fi

if [ ! -f LICENSE ]; then
  cat > LICENSE <<'EOF'
MIT License

Copyright (c) OwnHub contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
EOF
fi

if [ ! -f AGENTS.md ]; then
  cat > AGENTS.md <<'EOF'
# ownhub-extensions — agent guide

- One skill per `skills/<skill-id>/` folder; the folder name is the vault id.
- Every skill folder holds a `SKILL.md` with `name:` + `description:` frontmatter.
- Keep skill folders self-contained; shared helpers live in `skills/<id>/scripts/`.
- One MCP per `mcps/<mcp-id>/` package with a `pyproject.toml` console script.
- Never commit secrets; MCP env keys stay operator-side.
EOF
fi

# shellcheck disable=SC2086
for skill in $SKILLS; do
  if [ ! -f "skills/$skill/SKILL.md" ]; then
    mkdir -p "skills/$skill/scripts"
    cat > "skills/$skill/SKILL.md" <<EOF
---
name: $skill
description: TODO: one-line description for the $skill skill.
---

# $skill

TODO: instructions for this skill.
EOF
  fi
done

# shellcheck disable=SC2086
for mcp in $MCPS; do
  cmd="$(printf '%s' "$mcp" | sed 's/^ownhub-//')"
  case "$mcp" in
    ownhub-cloudflare) cmd="cloudflare-mcp" ;;
  esac
  if [ ! -f "mcps/$mcp/pyproject.toml" ]; then
    mkdir -p "mcps/$mcp"
    cat > "mcps/$mcp/pyproject.toml" <<EOF
[project]
name = "$mcp"
version = "0.1.0"
description = "TODO: describe the $mcp MCP server."
requires-python = ">=3.10"

[project.scripts]
$cmd = "$mcp:main"
EOF
    touch "mcps/$mcp/README.md"
  fi
done

if [ "$WITH_SUBTREES" -eq 1 ]; then
  if [ ! -d .git ]; then
    git init -q .
  fi
  echo "Pulling standalone repos as subtrees (best effort)..."
  # shellcheck disable=SC2086
  for skill in $SKILLS; do
    legacy="$(printf '%s' "$skill" | sed 's/^ownhub-//')"
    case "$skill" in
      ownhub) repo="$ORG/ownhub.git" ;;
      ownhub-smart-build-router) continue ;;
      *) repo="$ORG/$legacy.git" ;;
    esac
    if [ ! -f "skills/$skill/SKILL.md" ] || grep -q "TODO: one-line" "skills/$skill/SKILL.md"; then
      git subtree pull --prefix "skills/$skill" "$repo" main --squash 2>/dev/null || \
        echo "  skip: $repo not reachable"
    fi
  done
fi

echo "Monorepo scaffold ready at $DEST"
