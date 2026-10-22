#!/bin/sh
set -e

# OrquestraAI Official Secure Installer
# https://github.com/internetdozero/OrquestraAI
#
# Trust model: the release's SHA256SUMS.txt carries a detached Minisign /
# Ed25519 signature (SHA256SUMS.txt.minisig) from the offline release key.
# The installer verifies that signature BEFORE trusting any checksum, so a
# compromised release page serving a forged binary + matching forged
# checksums still fails closed. Same model as the in-app updater
# (src-tauri/src/update_sig.rs); see docs/security.md for the key and
# manual verification commands.
REPO="internetdozero/OrquestraAI"
# Embedded release public key (minisign .pub key-id + base64 body). The
# matching secret lives only in the maintainer's offline copy and the
# UPDATE_SIGNING_SEED CI secret. Must equal
# src-tauri/src/update_sig.rs::RELEASE_PUBLIC_KEY.
RELEASE_PUBKEY="RWRXcQWxu51A3hiNwiOi7zV6Dm+uUXR4zZrg1IbsZqD8RAqHZwgKbeU1"
BIN_DIR="${HOME}/.local/bin"
TARGET_BIN="${BIN_DIR}/orquestra"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/512x512/apps"

# 0. CLI Flags
REQUIRE_SIG=0
for arg in "$@"; do
    case "$arg" in
        --require-sig) REQUIRE_SIG=1 ;;
        -h|--help)
            echo "Usage: install.sh [--require-sig]"
            echo "  --require-sig  Fail unless the release signature verifies"
            echo "                 (without a verifier or .minisig asset)."
            exit 0 ;;
        *) echo "❌ Error: unknown option '$arg' (supported: --require-sig)." >&2; exit 1 ;;
    esac
done

# 1. Dependency Checks
command -v curl >/dev/null 2>&1 || {
    echo "❌ Error: 'curl' is required but not installed." >&2
    exit 1
}

command -v mktemp >/dev/null 2>&1 || {
    echo "❌ Error: 'mktemp' is required but not installed." >&2
    exit 1
}

# Determine available SHA-256 verification tool
if command -v sha256sum >/dev/null 2>&1; then
    HASH_CMD="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
    HASH_CMD="shasum -a 256"
else
    echo "❌ Error: Neither 'sha256sum' nor 'shasum' is available for checksum verification." >&2
    exit 1
fi

# 2. Secure Temporary Workspace & Cleanup Trap
mkdir -p "$BIN_DIR"
BASE_TMP="${TMPDIR:-/tmp}"
[ -d "$BASE_TMP" ] && [ -w "$BASE_TMP" ] || BASE_TMP="/var/tmp"
TMP_DIR=$(mktemp -d "${BASE_TMP}/orquestra-install.XXXXXXXXXX")
chmod 700 "$TMP_DIR"

STAGING_BIN=""
cleanup() {
    exit_code=$?
    rm -rf "$TMP_DIR" 2>/dev/null || true
    if [ -n "$STAGING_BIN" ] && [ -f "$STAGING_BIN" ]; then
        rm -f "$STAGING_BIN" 2>/dev/null || true
    fi
    exit "$exit_code"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# 2b. Platform Detection (OS + architecture -> release asset)
# Mirrors the in-app updater mapping in src-tauri/src/updater.rs:
# Linux ships `orquestra-linux-x86_64` (plus the legacy `orquestra-ai`
# alias); macOS ships `orquestra-macos-arm64` (Apple Silicon only).
OS="$(uname -s)"
ARCH="$(uname -m)"
ASSET_CANDIDATES=""
case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64) ASSET_CANDIDATES="orquestra-linux-x86_64 orquestra-ai" ;;
            *) echo "❌ Error: unsupported architecture '$ARCH' on Linux (only x86_64 has release binaries)." >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            arm64|aarch64) ASSET_CANDIDATES="orquestra-macos-arm64" ;;
            *) echo "❌ Error: unsupported architecture '$ARCH' on macOS (only arm64/Apple Silicon has release binaries)." >&2; exit 1 ;;
        esac
        ;;
    *) echo "❌ Error: unsupported operating system '$OS' (only Linux and macOS are supported)." >&2; exit 1 ;;
esac

echo "🔒 Downloading OrquestraAI release & SHA-256 checksum..."
echo "ℹ️  Platform: ${OS}/${ARCH}"

# 3. Fetch Release Binary and Checksums
RELEASE_URL="https://github.com/${REPO}/releases/latest/download"
ASSET=""
for candidate in $ASSET_CANDIDATES; do
    if curl -fsSL "${RELEASE_URL}/${candidate}" -o "${TMP_DIR}/${candidate}" 2>/dev/null; then
        ASSET="$candidate"
        break
    fi
done
if [ -z "$ASSET" ]; then
    echo "❌ Error: Could not download a release binary for ${OS}/${ARCH} (tried: ${ASSET_CANDIDATES})." >&2
    exit 1
fi
echo "ℹ️  Release asset: ${ASSET}"
curl -fsSL "${RELEASE_URL}/SHA256SUMS.txt" -o "${TMP_DIR}/SHA256SUMS.txt"
# Older releases predate signing; a missing asset degrades to the
# no-verifier path below (warning, or hard fail with --require-sig).
curl -fsSL "${RELEASE_URL}/SHA256SUMS.txt.minisig" -o "${TMP_DIR}/SHA256SUMS.txt.minisig" 2>/dev/null || true

# 3b. Detached-signature gate over the exact SHA256SUMS.txt bytes.
# Runs BEFORE any checksum is trusted: without the offline secret key no
# valid signature can exist for forged checksums. Verification order:
# reference `minisign`, then an inline PyNaCl prehashed check (same math
# as scripts/sign-release.py and update_sig.rs), then a loud warning
# that --require-sig turns into a hard failure.
SIG_STATUS=""
if [ -s "${TMP_DIR}/SHA256SUMS.txt" ] && [ -s "${TMP_DIR}/SHA256SUMS.txt.minisig" ]; then
    if command -v minisign >/dev/null 2>&1; then
        if minisign -Vm "${TMP_DIR}/SHA256SUMS.txt" -P "$RELEASE_PUBKEY" \
                -x "${TMP_DIR}/SHA256SUMS.txt.minisig"; then
            SIG_STATUS="verified (minisign)"
        else
            echo "❌ Security Error: release signature INVALID — refusing an unsigned release." >&2
            exit 1
        fi
    elif command -v python3 >/dev/null 2>&1 && python3 -c "import nacl.signing" 2>/dev/null; then
        if python3 - "$RELEASE_PUBKEY" "${TMP_DIR}/SHA256SUMS.txt" \
                "${TMP_DIR}/SHA256SUMS.txt.minisig" << 'PYEOF'; then
import base64, hashlib, sys
from nacl.signing import VerifyKey
from nacl.exceptions import BadSignatureError
pub = base64.b64decode(sys.argv[1])
sums = open(sys.argv[2], 'rb').read()
lines = open(sys.argv[3]).read().splitlines()
sig = base64.b64decode(lines[1].strip())
if sig[2:10] != pub[2:10]:
    sys.exit(1)
digest = hashlib.blake2b(sums, digest_size=64).digest()
try:
    VerifyKey(pub[10:42]).verify(sig[10:74] + digest)
except BadSignatureError:
    sys.exit(1)
PYEOF
            SIG_STATUS="verified (pynacl prehashed)"
        else
            echo "❌ Security Error: release signature INVALID — refusing an unsigned release." >&2
            exit 1
        fi
    elif [ "$REQUIRE_SIG" = "1" ]; then
        echo "❌ Security Error: --require-sig given but no verifier available (install minisign or python3-pynacl)." >&2
        exit 1
    else
        echo "⚠️  Warning: no signature verifier found (minisign or python3 with PyNaCl);" >&2
        echo "   proceeding on SHA-256 alone. Prefer '--require-sig' on an equipped host." >&2
    fi
elif [ "$REQUIRE_SIG" = "1" ]; then
    echo "❌ Security Error: --require-sig given but SHA256SUMS.txt(.minisig) is missing or empty." >&2
    exit 1
else
    echo "⚠️  Warning: release carries no verifiable signature asset; proceeding on SHA-256 alone." >&2
fi
if [ -n "$SIG_STATUS" ]; then
    echo "✓ Release signature $SIG_STATUS: checksums authentic."
fi

# 4. Strict Cryptographic Integrity Check
if [ ! -s "${TMP_DIR}/SHA256SUMS.txt" ]; then
    echo "❌ Security Error: SHA256SUMS.txt is missing or empty." >&2
    exit 1
fi

# Extract exactly the 64-hex SHA-256 entry for the downloaded asset
CHECKSUM_ENTRY=$(grep -E "^[a-fA-F0-9]{64}[[:space:]]+(\./)?${ASSET}$" "${TMP_DIR}/SHA256SUMS.txt" || true)

if [ -z "$CHECKSUM_ENTRY" ]; then
    echo "❌ Security Error: No valid SHA-256 entry found for ${ASSET}." >&2
    exit 1
fi

# Verify entry uniqueness to prevent spoofing/collisions
MATCH_COUNT=$(printf "%s\n" "$CHECKSUM_ENTRY" | wc -l | tr -d ' ')
if [ "$MATCH_COUNT" -ne 1 ]; then
    echo "❌ Security Error: Ambiguous checksum entries found (${MATCH_COUNT})." >&2
    exit 1
fi

printf "%s\n" "$CHECKSUM_ENTRY" > "${TMP_DIR}/CHECKSUM.txt"

echo "🔍 Verifying binary authenticity with SHA-256..."
(cd "$TMP_DIR" && $HASH_CMD --check --status CHECKSUM.txt) || {
    echo "❌ Security Error: SHA-256 checksum verification failed!" >&2
    echo "The downloaded binary does not match the official release hash." >&2
    exit 1
}
echo "✓ SHA-256 integrity check passed: binary authentic."

# 5. Truly Atomic Installation (Same-Filesystem Staging & Rename)
chmod 755 "${TMP_DIR}/${ASSET}"
STAGING_BIN=$(mktemp "${BIN_DIR}/.orquestra.tmp.XXXXXXXXXX")
cp -p "${TMP_DIR}/${ASSET}" "$STAGING_BIN"
chmod 755 "$STAGING_BIN"

# Atomic swap: rename(2) guarantees seamless, atomic replacement
mv -f "$STAGING_BIN" "$TARGET_BIN"
STAGING_BIN=""

# 6. Desktop Integration & Icons (Linux only: macOS has no XDG launchers
# and the app bundle carries its own icon, so this section is skipped there)
if [ "$OS" = "Linux" ]; then
    mkdir -p "$APP_DIR" "$ICON_DIR" "${HOME}/.local/share/icons"

    # Clean up legacy 1024x1024 icons from early alpha builds that override theme colors
    rm -f "${HOME}/.local/share/icons/hicolor/1024x1024/apps/orquestra.png" 2>/dev/null || true
    rm -f "${HOME}/.local/share/icons/hicolor/1024x1024/apps/orquestra-ai.png" 2>/dev/null || true

    # Pinned icon: resolve the latest release tag first and fetch the icon
    # at that tag, never the moving `main` branch. A miss is non-fatal: the
    # app installs its bundled icons on first launch anyway.
    ICON_TMP="${TMP_DIR}/icon.png"
    RELEASE_TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
        | grep -o '"tag_name": *"[^"]*"' | head -n 1 | cut -d'"' -f4 || true)
    if [ -n "$RELEASE_TAG" ] && curl -fsSL "https://raw.githubusercontent.com/${REPO}/${RELEASE_TAG}/src-tauri/icons/icon.png" -o "$ICON_TMP" 2>/dev/null && [ -s "$ICON_TMP" ]; then
        cp "$ICON_TMP" "${ICON_DIR}/orquestra.png"
        cp "$ICON_TMP" "${ICON_DIR}/orquestra-ai.png"
        cp "$ICON_TMP" "${HOME}/.local/share/icons/orquestra.png" 2>/dev/null || true
        cp "$ICON_TMP" "${HOME}/.local/share/icons/orquestra-ai.png" 2>/dev/null || true
    else
        echo "ℹ️  Notice: Could not download application icon; existing icons preserved."
    fi

    cat << 'EOF' > "${APP_DIR}/orquestra.desktop"
[Desktop Entry]
Name=OrquestraAI
GenericName=AI Harness Control Plane
Comment=Universal AI Harness Administration Platform
Exec=orquestra %U
Icon=orquestra-ai
Terminal=false
Type=Application
Categories=Development;Utility;System;
StartupNotify=true
StartupWMClass=orquestra
Keywords=ai;harness;skills;mcp;agents;codex;claude;gemini;orquestra;
EOF
    chmod 644 "${APP_DIR}/orquestra.desktop"

    # Alias entries stay hidden: without NoDisplay, GNOME/KDE/XFCE menus
    # show two or three duplicate OrquestraAI icons.
    for alias in orquestra-ai.desktop ai.orquestra.desktop.desktop; do
        cp "${APP_DIR}/orquestra.desktop" "${APP_DIR}/${alias}" 2>/dev/null || true
        chmod 644 "${APP_DIR}/${alias}" 2>/dev/null || true
        grep -qx 'NoDisplay=true' "${APP_DIR}/${alias}" 2>/dev/null || \
            printf 'NoDisplay=true\n' >> "${APP_DIR}/${alias}"
    done

    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true
    fi
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$APP_DIR" 2>/dev/null || true
    fi
    if command -v kbuildsycoca6 >/dev/null 2>&1; then
        kbuildsycoca6 --noincremental 2>/dev/null || true
    elif command -v kbuildsycoca5 >/dev/null 2>&1; then
        kbuildsycoca5 --noincremental 2>/dev/null || true
    fi
fi

echo "========================================================"
echo "🎉 OrquestraAI installed successfully to ${TARGET_BIN}"
echo "Run 'orquestra' in your terminal to launch the control plane."
echo "========================================================"

