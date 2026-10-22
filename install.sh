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
ALLOW_UNSIGNED_LEGACY=0
for arg in "$@"; do
    case "$arg" in
        --require-sig) ALLOW_UNSIGNED_LEGACY=0 ;;
        --allow-unsigned-legacy) ALLOW_UNSIGNED_LEGACY=1 ;;
        -h|--help)
            echo "Usage: install.sh [--require-sig | --allow-unsigned-legacy]"
            echo "  --require-sig             Require a verifiable release signature (default)"
            echo "  --allow-unsigned-legacy   Allow checksum-only verification for old unsigned releases"
            exit 0 ;;
        *) echo "❌ Error: unknown option '$arg' (supported: --require-sig, --allow-unsigned-legacy)." >&2; exit 1 ;;
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
# alias); macOS ships `orquestra-macos-arm64` and `orquestra-macos-x86_64`.
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
            x86_64) ASSET_CANDIDATES="orquestra-macos-x86_64" ;;
            *) echo "❌ Error: unsupported architecture '$ARCH' on macOS (only arm64 and x86_64 have release binaries)." >&2; exit 1 ;;
        esac
        ;;
    *) echo "❌ Error: unsupported operating system '$OS' (only Linux and macOS are supported)." >&2; exit 1 ;;
esac

echo "🔒 Downloading OrquestraAI release & SHA-256 checksum..."
echo "ℹ️  Platform: ${OS}/${ARCH}"

# 3. Fetch Release Binary and Checksums
LATEST_RELEASE_URL="https://github.com/${REPO}/releases/latest"
EFFECTIVE_SUMS_URL=$(curl -fsSL -w '%{url_effective}' \
    "${LATEST_RELEASE_URL}/download/SHA256SUMS.txt" \
    -o "${TMP_DIR}/SHA256SUMS.txt") || {
    echo "❌ Error: Could not download release checksums." >&2
    exit 1
}
RELEASE_TAG=$(printf '%s' "$EFFECTIVE_SUMS_URL" | sed -n \
    's#^.*/releases/download/\([^/]*\)/SHA256SUMS\.txt$#\1#p')
case "$RELEASE_TAG" in
    ""|*[!A-Za-z0-9._+-]*)
        echo "❌ Security Error: could not safely resolve the release tag." >&2
        exit 1
        ;;
esac
RELEASE_VERSION=${RELEASE_TAG#v}
RELEASE_URL="https://github.com/${REPO}/releases/download/${RELEASE_TAG}"
echo "ℹ️  Release tag: ${RELEASE_TAG}"
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
# Older releases predate signing. They require the explicit
# --allow-unsigned-legacy opt-in; current releases fail closed by default.
SIGNATURE_AVAILABLE=1
if ! curl -fsSL "${RELEASE_URL}/SHA256SUMS.txt.minisig" \
        -o "${TMP_DIR}/SHA256SUMS.txt.minisig" 2>/dev/null ||
        [ ! -s "${TMP_DIR}/SHA256SUMS.txt.minisig" ]; then
    SIGNATURE_AVAILABLE=0
    if [ "$ALLOW_UNSIGNED_LEGACY" != "1" ]; then
        echo "❌ Security Error: release signature is missing; unsigned legacy releases require --allow-unsigned-legacy." >&2
        exit 1
    fi
    echo "⚠️  Warning: explicitly allowing an unsigned legacy release; SHA-256 is the only integrity check." >&2
fi

verify_trusted_comment() {
    trusted_comment=$(sed -n 's/^trusted comment: //p' "$TMP_DIR/SHA256SUMS.txt.minisig")
    tab=$(printf '\t')
    printf '%s\n' "$trusted_comment" | awk -F "$tab" -v version="$RELEASE_VERSION" \
        '$1 ~ /^timestamp=[0-9][0-9]*$/ && $2 == "file=SHA256SUMS.txt" && $3 == "version=" version && NF == 3 { valid=1 } END { exit !valid }'
}

# 3b. Detached-signature gate over the exact SHA256SUMS.txt bytes.
# Runs BEFORE any checksum is trusted: without the offline secret key no
# valid signature can exist for forged checksums. Verification order:
# reference `minisign`, then an inline PyNaCl prehashed check (same math
# as scripts/sign-release.py and update_sig.rs).
SIG_STATUS=""
if [ "$SIGNATURE_AVAILABLE" = "1" ]; then
    if command -v minisign >/dev/null 2>&1; then
        if minisign -Vm "${TMP_DIR}/SHA256SUMS.txt" -P "$RELEASE_PUBKEY" \
                -x "${TMP_DIR}/SHA256SUMS.txt.minisig"; then
            if ! verify_trusted_comment; then
                echo "❌ Security Error: signature metadata does not match this release." >&2
                exit 1
            fi
            SIG_STATUS="verified (minisign)"
        else
            echo "❌ Security Error: release signature INVALID — refusing an unsigned release." >&2
            exit 1
        fi
    elif command -v python3 >/dev/null 2>&1 && python3 -c "import nacl.signing" 2>/dev/null; then
        if python3 - "$RELEASE_PUBKEY" "${TMP_DIR}/SHA256SUMS.txt" \
                "${TMP_DIR}/SHA256SUMS.txt.minisig" "$RELEASE_VERSION" << 'PYEOF'; then
import base64, hashlib, sys
from nacl.signing import VerifyKey
pub = base64.b64decode(sys.argv[1])
sums = open(sys.argv[2], 'rb').read()
lines = open(sys.argv[3]).read().splitlines()
version = sys.argv[4]
if len(lines) != 4 or not lines[2].startswith('trusted comment: '):
    sys.exit(1)
sig = base64.b64decode(lines[1].strip(), validate=True)
trusted = lines[2][len('trusted comment: '):]
global_sig = base64.b64decode(lines[3].strip(), validate=True)
if len(pub) != 42 or len(sig) != 74 or sig[:2] != b'ED' or sig[2:10] != pub[2:10]:
    sys.exit(1)
fields = dict(field.split('=', 1) for field in trusted.split('\t') if '=' in field)
if (len(fields) != 3 or not fields.get('timestamp', '').isdigit()
        or fields.get('file') != 'SHA256SUMS.txt' or fields.get('version') != version):
    sys.exit(1)
digest = hashlib.blake2b(sums, digest_size=64).digest()
key = VerifyKey(pub[10:42])
key.verify(digest, sig[10:74])
key.verify(sig[10:74] + trusted.encode(), global_sig)
PYEOF
            SIG_STATUS="verified (pynacl prehashed)"
        else
            echo "❌ Security Error: release signature INVALID — refusing an unsigned release." >&2
            exit 1
        fi
    else
        echo "❌ Security Error: signed release cannot be verified; install minisign or python3-pynacl." >&2
        exit 1
    fi
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

    # Pinned icon: reuse the tag whose checksums and binary were verified,
    # never the moving `main` branch or a second latest-release lookup. A
    # miss is non-fatal: the app installs its bundled icons on first launch.
    ICON_TMP="${TMP_DIR}/icon.png"
    if curl -fsSL "https://raw.githubusercontent.com/${REPO}/${RELEASE_TAG}/src-tauri/icons/icon.png" -o "$ICON_TMP" 2>/dev/null && [ -s "$ICON_TMP" ]; then
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
