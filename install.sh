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

HASH_LINE=$(grep -E '[[:space:]]orquestra-ai$' "${TMP_DIR}/SHA256SUMS.txt" || true)

if [ -z "$HASH_LINE" ]; then
    echo "❌ Security Error: No checksum found for orquestra-ai in SHA256SUMS.txt."
    exit 1
fi

printf '%s\n' "$HASH_LINE" > "${TMP_DIR}/CHECKSUM.txt"

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

echo "========================================================"
echo "🎉 OrquestraAI installed successfully to ${TARGET_BIN}"
echo "Run 'orquestra' in your terminal to launch the control plane."
echo "========================================================"
