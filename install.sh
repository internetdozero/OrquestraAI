#!/bin/sh
set -e

# OrquestraAI Official Secure Installer
# https://github.com/internetdozero/OrquestraAI
REPO="internetdozero/OrquestraAI"
BIN_DIR="${HOME}/.local/bin"
TARGET_BIN="${BIN_DIR}/orquestra"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/512x512/apps"

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

