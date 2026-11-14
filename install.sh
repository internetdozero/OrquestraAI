#!/bin/sh
set -e

# OwnHub Official Secure Installer
# https://github.com/OwnAnAI/ownhub
#
# Trust model: the release's SHA256SUMS.txt carries a detached Minisign /
# Ed25519 signature (SHA256SUMS.txt.minisig) from the offline release key.
# The installer verifies that signature BEFORE trusting any checksum, so a
# compromised release page serving a forged binary + matching forged
# checksums still fails closed. Same model as the in-app updater
# (src-tauri/src/update_sig.rs); see docs/security.md for the key and
# manual verification commands.
REPO="OwnAnAI/ownhub"
# Embedded release public key (minisign .pub key-id + base64 body). The
# matching secret lives only in the maintainer's offline copy and the
# UPDATE_SIGNING_SEED CI secret. Must equal
# src-tauri/src/update_sig.rs::RELEASE_PUBLIC_KEY.
RELEASE_PUBKEY="RWRXcQWxu51A3hiNwiOi7zV6Dm+uUXR4zZrg1IbsZqD8RAqHZwgKbeU1"
BIN_DIR="${HOME}/.local/bin"
TARGET_BIN="${BIN_DIR}/ownhub"

# 0. CLI Flags
ALLOW_UNSIGNED_LEGACY=0
CHECK_DEPS_ONLY=0
AUTO_INSTALL_DEPS=1
for arg in "$@"; do
    case "$arg" in
        --require-sig) ALLOW_UNSIGNED_LEGACY=0 ;;
        --allow-unsigned-legacy) ALLOW_UNSIGNED_LEGACY=1 ;;
        --check-deps) CHECK_DEPS_ONLY=1 ;;
        --no-auto-deps) AUTO_INSTALL_DEPS=0 ;;
        -h|--help)
            echo "Usage: install.sh [--require-sig | --allow-unsigned-legacy | --check-deps | --no-auto-deps]"
            echo "  --require-sig             Require a verifiable release signature (default)"
            echo "  --allow-unsigned-legacy   Allow checksum-only verification for old unsigned releases"
            echo "  --check-deps              Check runtime libraries of the installed Linux binary without reinstalling"
            echo "  --no-auto-deps            Do not auto-install missing system dependencies"
            exit 0 ;;
        *) echo "❌ Error: unknown option '$arg' (supported: --require-sig, --allow-unsigned-legacy, --check-deps, --no-auto-deps)." >&2; exit 1 ;;
    esac
done

try_install_linux_deps() {
    missing_libs=$1; pkgs=""; mgr=""; cmd=""
    if command -v pacman >/dev/null 2>&1; then
        mgr="pacman"; cmd="pacman -S --needed --noconfirm"
        p_wk="webkit2gtk-4.1"; p_gtk="gtk3"; p_soup="libsoup3"; p_app="libayatana-appindicator"; p_rsvg="librsvg"
    elif command -v apt-get >/dev/null 2>&1; then
        mgr="apt-get"; cmd="apt-get install -y"
        p_wk="libwebkit2gtk-4.1-0"; p_gtk="libgtk-3-0"; p_soup="libsoup-3.0-0"; p_app="libayatana-appindicator3-1"; p_rsvg="librsvg2-2"
    elif command -v dnf >/dev/null 2>&1; then
        mgr="dnf"; cmd="dnf install -y"
        p_wk="webkit2gtk4.1"; p_gtk="gtk3"; p_soup="libsoup3"; p_app="libayatana-appindicator-gtk3"; p_rsvg="librsvg2"
    elif command -v zypper >/dev/null 2>&1; then
        mgr="zypper"; cmd="zypper --non-interactive install"
        p_wk="libwebkit2gtk-4_1-0"; p_gtk="libgtk-3-0"; p_soup="libsoup-3_0-0"; p_app=""; p_rsvg=""
    elif command -v apk >/dev/null 2>&1; then
        mgr="apk"; cmd="apk add"
        p_wk="webkit2gtk-4.1"; p_gtk="gtk+3.0"; p_soup="libsoup3"; p_app=""; p_rsvg=""
    else
        return 1
    fi
    for lib in $missing_libs; do
        case "$lib" in
            *webkit2gtk-4.1*|*javascriptcoregtk-4.1*) [ -n "$p_wk" ] && pkgs="$pkgs $p_wk" ;;
            *gtk-3*) [ -n "$p_gtk" ] && pkgs="$pkgs $p_gtk" ;;
            *soup-3*) [ -n "$p_soup" ] && pkgs="$pkgs $p_soup" ;;
            *appindicator*) [ -n "$p_app" ] && pkgs="$pkgs $p_app" ;;
            *rsvg*) [ -n "$p_rsvg" ] && pkgs="$pkgs $p_rsvg" ;;
        esac
    done
    dedup_pkgs=$(printf '%s\n' $pkgs | tr ' ' '\n' | sort -u | tr '\n' ' ' | sed 's/^ *//;s/ *$//')
    [ -z "$dedup_pkgs" ] && return 1
    SUDO=""
    if [ "$(id -u)" -ne 0 ]; then
        if command -v sudo >/dev/null 2>&1; then SUDO="sudo"; else return 1; fi
    fi
    echo "📦 Auto-installing missing system dependencies using ${mgr}: ${dedup_pkgs}"
    if [ "$mgr" = "apt-get" ]; then
        $SUDO apt-get update && $SUDO apt-get install -y $dedup_pkgs
    else
        $SUDO $cmd $dedup_pkgs
    fi
}

# Highest glibc symbol version the binary references, or empty when the
# requirement cannot be read. Mirrors scripts/check-glibc.sh, which CI runs
# against the built artifact; this copy exists because the installer is a
# single stream (`curl | sh`) and cannot depend on a sibling file.
required_glibc() {
    binary_path=$1
    if command -v objdump >/dev/null 2>&1; then
        symbols=$(objdump -T "$binary_path" 2>/dev/null || true)
    elif command -v readelf >/dev/null 2>&1; then
        symbols=$(readelf --dyn-syms --wide "$binary_path" 2>/dev/null || true)
    else
        return 0
    fi
    printf '%s\n' "$symbols" \
        | grep -o 'GLIBC_[0-9][0-9.]*' \
        | sed 's/^GLIBC_//' \
        | sort -u -V \
        | tail -1
}

# glibc a < glibc b? `sort -V` is the only version-aware comparison in POSIX
# userland, and both operands are already normalised `major.minor` text.
glibc_older() {
    [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -1)" = "$1" ] && [ "$1" != "$2" ]
}

# The loader stops at the first missing symbol version, so a binary built on a
# newer base is unusable here no matter which libraries are installed. `ldd`
# cannot see this: it reports libraries, not versions.
check_linux_glibc() {
    binary_path=$1
    [ "$(uname -s)" = "Linux" ] || return 0
    required=$(required_glibc "$binary_path")
    if [ -z "$required" ]; then
        echo "ℹ️  glibc version check skipped (no binutils, or not a dynamically linked binary)."
        return 0
    fi
    available=$(ldd --version 2>/dev/null | head -1 | grep -o '[0-9][0-9.]*$' || true)
    if [ -z "$available" ]; then
        echo "ℹ️  glibc version check skipped (could not read the local version)."
        return 0
    fi
    if glibc_older "$available" "$required"; then
        echo "❌ This binary requires glibc ${required}, but this system provides ${available}." >&2
        echo "   The loader would fail with 'GLIBC_${required} not found'." >&2
        echo "   Install the Flatpak bundle instead: it carries its own runtime, so the" >&2
        echo "   host glibc does not matter (docs/install.md explains the tradeoffs):" >&2
        echo "     curl -fLO https://github.com/${REPO}/releases/latest/download/OwnHub-x86_64.flatpak" >&2
        echo "     flatpak install --user ./OwnHub-x86_64.flatpak" >&2
        echo "   Building from source on this distribution also works." >&2
        return 1
    fi
    echo "✓ glibc ${available} can run this binary (needs ${required})."
}

# Commands the app shells out to. The binary starts without them, but the
# features they power fail at the first click, so diagnose them here instead
# of leaving the operator with a generic error inside the app.
# Argument: "check" reports only; "install" also installs when allowed.
ensure_linux_tools() {
    mode=$1
    missing=""
    if ! command -v xdg-open >/dev/null 2>&1 && ! command -v gio >/dev/null 2>&1; then
        missing="$missing opener"
    fi
    command -v git >/dev/null 2>&1 || missing="$missing git"
    python_ok=1
    command -v python3 >/dev/null 2>&1 || python_ok=0
    if [ "$python_ok" = "1" ] && ! python3 -c 'import ensurepip' >/dev/null 2>&1; then python_ok=0; fi
    if [ "$python_ok" = "1" ] && ! python3 -m venv --help >/dev/null 2>&1; then python_ok=0; fi
    [ "$python_ok" = "1" ] || missing="$missing python"

    [ -z "$missing" ] && return 0

    echo "⚠️  Optional runtime tools missing:${missing}" >&2
    echo "   They power: opening links/folders (xdg-utils or gio), skill sync/import (git)," >&2
    echo "   official MCP installs (python3 with venv/ensurepip)." >&2
    [ "$mode" = "check" ] && return 1

    if command -v pacman >/dev/null 2>&1; then
        mgr="pacman"; cmd="pacman -S --needed --noconfirm"; p_open="xdg-utils"; p_git="git"; p_py="python"
    elif command -v apt-get >/dev/null 2>&1; then
        mgr="apt-get"; cmd="apt-get install -y"; p_open="xdg-utils"; p_git="git"; p_py="python3 python3-venv"
    elif command -v dnf >/dev/null 2>&1; then
        mgr="dnf"; cmd="dnf install -y"; p_open="xdg-utils"; p_git="git"; p_py="python3"
    elif command -v zypper >/dev/null 2>&1; then
        mgr="zypper"; cmd="zypper --non-interactive install"; p_open="xdg-utils"; p_git="git"; p_py="python3"
    elif command -v apk >/dev/null 2>&1; then
        mgr="apk"; cmd="apk add"; p_open="xdg-utils"; p_git="git"; p_py="python3"
    else
        echo "   No supported package manager found; install them manually." >&2
        return 1
    fi

    pkgs=""
    for feature in $missing; do
        case "$feature" in
            opener) pkgs="$pkgs $p_open" ;;
            git) pkgs="$pkgs $p_git" ;;
            python) pkgs="$pkgs $p_py" ;;
        esac
    done
    SUDO=""
    if [ "$(id -u)" -ne 0 ]; then
        if command -v sudo >/dev/null 2>&1; then SUDO="sudo"; else
            echo "   Run as root to install them automatically." >&2
            return 1
        fi
    fi
    echo "📦 Installing missing runtime tools with ${mgr}:${pkgs}"
    if [ "$mgr" = "apt-get" ]; then
        $SUDO apt-get update && $SUDO apt-get install -y $pkgs
    else
        $SUDO $cmd $pkgs
    fi
}

check_linux_runtime_deps() {
    binary_path=$1
    if ! command -v ldd >/dev/null 2>&1; then
        echo "❌ Runtime dependency check unavailable: 'ldd' is not installed." >&2
        return 2
    fi
    if ldd_output=$(ldd "$binary_path" 2>&1); then
        :
    else
        echo "❌ Could not inspect runtime dependencies for ${binary_path}:" >&2
        printf '%s\n' "$ldd_output" >&2
        return 2
    fi
    if ! check_linux_glibc "$binary_path"; then
        return 1
    fi
    missing_libraries=$(printf '%s\n' "$ldd_output" | awk '/=> not found/ { print $1 }')
    if [ -n "$missing_libraries" ] && [ "$AUTO_INSTALL_DEPS" = "1" ]; then
        if try_install_linux_deps "$missing_libraries"; then
            if ldd_output=$(ldd "$binary_path" 2>&1); then
                missing_libraries=$(printf '%s\n' "$ldd_output" | awk '/=> not found/ { print $1 }')
            fi
        fi
    fi
    if [ -n "$missing_libraries" ]; then
        echo "❌ Missing runtime libraries for ${binary_path}:" >&2
        printf '%s\n' "$missing_libraries" | sed 's/^/  /' >&2
        echo "Install the packages that provide these libraries, then rerun this check." >&2
        if command -v pacman >/dev/null 2>&1; then
            echo "On Arch, use 'pacman -F <library-name>' to find each package." >&2
        fi
        return 1
    fi
    echo "✓ All linked runtime libraries are available."
}

if [ "$CHECK_DEPS_ONLY" = "1" ]; then
    [ "$(uname -s)" = "Linux" ] || { echo "❌ Runtime dependency diagnostics currently support Linux only." >&2; exit 2; }
    [ -x "$TARGET_BIN" ] || { echo "❌ Installed OwnHub binary not found at ${TARGET_BIN}." >&2; exit 1; }
    check_linux_runtime_deps "$TARGET_BIN" || exit "$?"
    ensure_linux_tools check || true
    exit 0
fi

# 1. Dependency Checks
command -v curl >/dev/null 2>&1 || { echo "❌ Error: 'curl' is required but not installed." >&2; exit 1; }
command -v mktemp >/dev/null 2>&1 || { echo "❌ Error: 'mktemp' is required but not installed." >&2; exit 1; }

# Determine available SHA-256 verification tool
if command -v sha256sum >/dev/null 2>&1; then HASH_CMD="sha256sum"
elif command -v shasum >/dev/null 2>&1; then HASH_CMD="shasum -a 256"
else echo "❌ Error: Neither 'sha256sum' nor 'shasum' is available for checksum verification." >&2; exit 1
fi

# Signature tool preflight. Both are optional at this point (the release may
# predate signing, and the verification step below reports precisely what is
# missing), but telling the operator which tool was found — and how to install
# one when none was — turns "install minisign or PyNaCl" into an actionable
# command instead of a dead end after the download.
SIG_TOOL=""
if command -v minisign >/dev/null 2>&1; then
    SIG_TOOL="minisign"
elif command -v python3 >/dev/null 2>&1 && python3 -c 'import nacl.signing' >/dev/null 2>&1; then
    SIG_TOOL="python3 (PyNaCl)"
fi
if [ -n "$SIG_TOOL" ]; then
    echo "ℹ️  Release signature will be verified with ${SIG_TOOL}."
else
    echo "⚠️  No signature verifier found: release signatures need 'minisign' or python3 with PyNaCl." >&2
    if command -v apt-get >/dev/null 2>&1; then
        echo "   Debian/Ubuntu: sudo apt-get install minisign   (or: python3-nacl)" >&2
    elif command -v dnf >/dev/null 2>&1; then
        echo "   Fedora/RHEL:   sudo dnf install minisign   (or: python3-pynacl)" >&2
    elif command -v pacman >/dev/null 2>&1; then
        echo "   Arch:          sudo pacman -S minisign   (or: python-pynacl)" >&2
    elif command -v brew >/dev/null 2>&1; then
        echo "   macOS:         brew install minisign" >&2
    fi
    echo "   The installer stops rather than skip signature verification." >&2
fi

# 2. Secure Temporary Workspace & Cleanup Trap
mkdir -p "$BIN_DIR"
BASE_TMP="${TMPDIR:-/tmp}"
[ -d "$BASE_TMP" ] && [ -w "$BASE_TMP" ] || BASE_TMP="/var/tmp"
TMP_DIR=$(mktemp -d "${BASE_TMP}/ownhub-install.XXXXXXXXXX")
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
# Linux ships `ownhub-linux-x86_64` (plus the legacy `ownhub`
# alias); macOS ships `ownhub-macos-arm64` and `ownhub-macos-x86_64`.
OS="$(uname -s)"
ARCH="$(uname -m)"
ASSET_CANDIDATES=""
case "${OS}-${ARCH}" in
    Linux-x86_64) ASSET_CANDIDATES="ownhub-linux-x86_64 ownhub" ;;
    # The signed bundle is preferred: it is the artifact the release signs and
    # notarizes, and macOS treats a bare Mach-O from the internet as an
    # unidentified developer. The bare binary stays as the fallback for
    # releases built without the bundle step.
    Darwin-arm64|Darwin-aarch64) ASSET_CANDIDATES="OwnHub-macos-arm64.app.zip ownhub-macos-arm64" ;;
    Darwin-x86_64) ASSET_CANDIDATES="OwnHub-macos-x86_64.app.zip ownhub-macos-x86_64" ;;
    *) echo "❌ Error: unsupported platform ${OS}/${ARCH}." >&2; exit 1 ;;
esac

echo "🔒 Downloading OwnHub release & SHA-256 checksum..."
echo "ℹ️  Platform: ${OS}/${ARCH}"

# 3. Fetch Release Binary and Checksums
LATEST_RELEASE_URL="https://github.com/${REPO}/releases/latest"
# Resolve the tag from the first redirect of /releases/latest (not the final
# URL: release assets are now served from release-assets.githubusercontent.com,
# so %{url_effective} no longer contains the tag).
LATEST_TAG_URL=$(curl -fsS --proto '=https' -o /dev/null -w '%{redirect_url}' \
    "${LATEST_RELEASE_URL}") || {
    echo "❌ Error: Could not resolve the latest release." >&2
    exit 1
}
RELEASE_TAG=$(printf '%s' "$LATEST_TAG_URL" | sed -n \
    "s#^https://github\\.com/${REPO}/releases/tag/\\([^/?#]*\\)\$#\\1#p")
case "$RELEASE_TAG" in
    ""|*[!A-Za-z0-9._+-]*)
        echo "❌ Security Error: could not safely resolve the release tag." >&2
        exit 1
        ;;
esac
RELEASE_VERSION=${RELEASE_TAG#v}
RELEASE_URL="https://github.com/${REPO}/releases/download/${RELEASE_TAG}"
curl -fsSL "${RELEASE_URL}/SHA256SUMS.txt" -o "${TMP_DIR}/SHA256SUMS.txt" || {
    echo "❌ Error: Could not download release checksums." >&2
    exit 1
}
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

# Check dynamic Linux libraries before replacing an existing installation.
# The binary's detached release signature has already been verified above.
if [ "$OS" = "Linux" ]; then
    echo "🔍 Checking Linux runtime libraries..."
    if ! check_linux_runtime_deps "${TMP_DIR}/${ASSET}"; then
        echo "❌ Installation stopped before replacing the installed binary." >&2
        exit 1
    fi
    if [ "$AUTO_INSTALL_DEPS" = "1" ]; then
        ensure_linux_tools install || true
    fi
fi

# 5. Installation
#
# Two shapes reach this point: the bare executable every platform publishes
# (which the in-app updater replaces in place) and, on macOS, the signed
# `OwnHub.app` bundle. Both are installed from the staging copy whose
# SHA-256 was just verified.
install_binary() {
    source_bin=$1
    chmod 755 "$source_bin"
    STAGING_BIN=$(mktemp "${BIN_DIR}/.ownhub.tmp.XXXXXXXXXX")
    cp -p "$source_bin" "$STAGING_BIN"
    chmod 755 "$STAGING_BIN"

    # Atomic swap: rename(2) guarantees seamless, atomic replacement
    mv -f "$STAGING_BIN" "$TARGET_BIN"
    STAGING_BIN=""
}

# Install a notarized bundle into ~/Applications and point the `ownhub`
# command at the executable inside it.
#
# The bundle is copied into a sibling staging directory and swapped in, so an
# interrupted install leaves the previous app intact. Quarantine is cleared
# because the archive was downloaded by this script rather than by a browser;
# a signed bundle does not need that, but an unsigned build does.
install_macos_bundle() {
    archive=$1
    apps_dir="${HOME}/Applications"
    bundle="${apps_dir}/OwnHub.app"
    unpack="${TMP_DIR}/unpacked"

    command -v unzip >/dev/null 2>&1 || {
        echo "❌ Error: 'unzip' is required to install the macOS app bundle." >&2
        exit 1
    }
    rm -rf "$unpack"
    mkdir -p "$unpack"
    unzip -q "$archive" -d "$unpack" || {
        echo "❌ Error: the bundle archive could not be extracted." >&2
        exit 1
    }

    app=$(find "$unpack" -maxdepth 2 -name '*.app' -type d | head -1)
    if [ -z "$app" ] || [ ! -d "$app/Contents/MacOS" ]; then
        echo "❌ Error: the archive does not contain an app bundle." >&2
        exit 1
    fi

    mkdir -p "$apps_dir"
    staged="${apps_dir}/.OwnHub.app.new.$$"
    rm -rf "$staged"
    cp -R "$app" "$staged" || {
        echo "❌ Error: could not stage the app bundle in ${apps_dir}." >&2
        exit 1
    }
    if command -v xattr >/dev/null 2>&1; then
        xattr -dr com.apple.quarantine "$staged" 2>/dev/null || true
    fi
    rm -rf "$bundle"
    mv "$staged" "$bundle"

    # The CLI keeps its documented name. The link is what `ownhub` resolves
    # through, and the updater replaces the real Mach-O behind it.
    executable=$(find "$bundle/Contents/MacOS" -maxdepth 1 -type f | head -1)
    if [ -z "$executable" ]; then
        echo "❌ Error: the app bundle has no executable." >&2
        exit 1
    fi
    mkdir -p "$BIN_DIR"
    rm -f "$TARGET_BIN"
    ln -s "$executable" "$TARGET_BIN"
    TARGET_BIN="$bundle"
}

case "$ASSET" in
    *.app.zip) install_macos_bundle "${TMP_DIR}/${ASSET}" ;;
    *) install_binary "${TMP_DIR}/${ASSET}" ;;
esac

# 6. Desktop integration is owned by the app itself.
# `src-tauri/src/desktop_icons.rs` writes the launcher and every icon size on
# each launch (honouring XDG_DATA_HOME) and refreshes the desktop caches only
# when the bytes changed. Installed here, the same files would be written twice
# from two sources that drift apart, so this script no longer touches them.

echo "========================================================"
echo "🎉 OwnHub installed successfully to ${TARGET_BIN}"
echo "Run 'ownhub' in your terminal to launch the control plane."
if [ "$OS" = "Darwin" ]; then
    case "$ASSET" in
        *.app.zip)
            echo "Open it from Launchpad, Spotlight, or with 'open -a OwnHub'."
            ;;
        *)
            # A bare Mach-O downloaded over the network is quarantined; the
            # signed bundle avoids this, which is why it is preferred above.
            echo "macOS quarantines a binary downloaded from the internet. If it is blocked,"
            echo "allow it once in System Settings → Privacy & Security, or clear the flag:"
            echo "  xattr -d com.apple.quarantine ${TARGET_BIN}"
            ;;
    esac
fi
if [ "$OS" = "Linux" ]; then
    echo "If it later fails to launch, diagnose missing libraries without reinstalling:"
    echo "  curl -fsSL https://ownhub.internetdozero.com.br/install.sh | sh -s -- --check-deps"
fi
echo "========================================================"
