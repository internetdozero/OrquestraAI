#!/bin/sh
set -e

# OrquestraAI Official Secure Installer
REPO="internetdozero/OrquestraAI"
BIN_DIR="${HOME}/.local/bin"
TARGET_BIN="${BIN_DIR}/orquestra"
TMP_DIR="/var/tmp/testing/orquestra-install-$$"

mkdir -p "$BIN_DIR"
mkdir -p "$TMP_DIR"
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

echo "🔒 Downloading OrquestraAI release & SHA-256 checksum..."

# Fetch release binary and checksums
curl -fsSL "https://github.com/${REPO}/releases/latest/download/orquestra-ai" -o "${TMP_DIR}/orquestra-ai"
curl -fsSL "https://github.com/${REPO}/releases/latest/download/SHA256SUMS.txt" -o "${TMP_DIR}/SHA256SUMS.txt"

# Cryptographic Integrity Check
if [ ! -s "${TMP_DIR}/SHA256SUMS.txt" ]; then
    echo "❌ Security Error: SHA256SUMS.txt is missing from release."
    exit 1
fi

MATCH_COUNT=$(grep -Ec '[[:space:]]orquestra-ai$' "${TMP_DIR}/SHA256SUMS.txt" || true)

if [ "$MATCH_COUNT" -ne 1 ]; then
    echo "❌ Security Error: Expected exactly one checksum entry for orquestra-ai, found ${MATCH_COUNT}."
    exit 1
fi

grep -E '[[:space:]]orquestra-ai$' "${TMP_DIR}/SHA256SUMS.txt" > "${TMP_DIR}/CHECKSUM.txt"

echo "🔍 Verifying binary authenticity with SHA-256..."
(cd "$TMP_DIR" && sha256sum --check --status CHECKSUM.txt) || {
    echo "❌ Security Error: SHA-256 checksum verification failed!"
    echo "The downloaded binary does not match the official release hash."
    exit 1
}
echo "✓ SHA-256 integrity check passed: binary authentic."

# Install atomically
mv "${TMP_DIR}/orquestra-ai" "$TARGET_BIN"
chmod +x "$TARGET_BIN"

# Set up desktop launcher & icons for Wayland and X11 docks
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/512x512/apps"
mkdir -p "$APP_DIR" "$ICON_DIR"

curl -fsSL "https://raw.githubusercontent.com/${REPO}/main/src-tauri/icons/icon.png" -o "${ICON_DIR}/orquestra.png" 2>/dev/null || true
cp "${ICON_DIR}/orquestra.png" "${ICON_DIR}/orquestra-ai.png" 2>/dev/null || true
cp "${ICON_DIR}/orquestra.png" "${HOME}/.local/share/icons/orquestra.png" 2>/dev/null || true
cp "${ICON_DIR}/orquestra.png" "${HOME}/.local/share/icons/orquestra-ai.png" 2>/dev/null || true

cat << 'EOF' > "${APP_DIR}/orquestra.desktop"
[Desktop Entry]
Name=OrquestraAI
GenericName=AI Harness Control Plane
Comment=Universal AI Harness Administration Platform
Exec=orquestra %U
Icon=orquestra
Terminal=false
Type=Application
Categories=Development;Utility;System;
StartupNotify=true
StartupWMClass=orquestra
Keywords=ai;harness;skills;mcp;agents;codex;claude;gemini;orquestra;
EOF
cp "${APP_DIR}/orquestra.desktop" "${APP_DIR}/orquestra-ai.desktop" 2>/dev/null || true

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" 2>/dev/null || true
fi

echo "========================================================"
echo "🎉 OrquestraAI installed successfully to ${TARGET_BIN}"
echo "Run 'orquestra' in your terminal to launch the control plane."
echo "========================================================"
