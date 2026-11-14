#!/usr/bin/env bash
# Build the self-contained Linux bundle from a release binary.
#
# The published Linux binary is linked against glibc 2.35 and WebKitGTK 4.1
# (built on Ubuntu 22.04). Distributions below that baseline cannot load it,
# and no package install raises a symbol version — see docs/install.md. This
# script wraps that exact binary in a Flatpak, which runs it inside the GNOME
# runtime and therefore supplies glibc, GTK 3 and WebKitGTK itself.
#
# Usage: scripts/build-flatpak.sh [--stage-only] <binary> [output-file]
#
#   --stage-only   copy the binary, desktop file, metainfo and icons into
#                  packaging/flatpak/files/ and stop. CI uses this to let the
#                  Flathub builder action run flatpak-builder itself, which
#                  ships 1.4 instead of Ubuntu 22.04's 1.2 (the old builder
#                  calls the `appstream-compose` binary that GNOME SDK 47
#                  no longer contains).
#   <binary>       the artifact to wrap (`ownhub-linux-x86_64` from the
#                  release, or target/release/ownhub)
#   [output-file]  defaults to ./OwnHub-x86_64.flatpak
#
# Requires flatpak + flatpak-builder and the GNOME runtime/SDK the manifest
# names. CI installs them; locally:
#
#   flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47
#
# Builds nothing: the binary is copied, so the bundle carries the same bytes
# the release already signs and checksums.
set -euo pipefail

stage_only=0
if [ "${1:-}" = "--stage-only" ]; then
    stage_only=1
    shift
fi

binary=${1:?usage: scripts/build-flatpak.sh [--stage-only] <binary> [output-file]}
output=${2:-OwnHub-x86_64.flatpak}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
manifest_dir="$root/packaging/flatpak"
manifest="$manifest_dir/dev.ownanai.hub.yml"
app_id=dev.ownanai.hub
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$root/src-tauri/Cargo.toml" | head -1)

[ -f "$binary" ] || {
    echo "error: $binary does not exist" >&2
    exit 1
}
[ -n "$version" ] || {
    echo "error: could not read the version from src-tauri/Cargo.toml" >&2
    exit 1
}
if [ "$stage_only" = 0 ]; then
    for tool in flatpak flatpak-builder; do
        command -v "$tool" >/dev/null 2>&1 || {
            echo "error: $tool is not installed (Flatpak packaging needs both)" >&2
            echo "       Debian/Ubuntu: sudo apt-get install -y flatpak flatpak-builder" >&2
            echo "       Arch:          sudo pacman -S flatpak flatpak-builder" >&2
            exit 1
        }
    done
    # `flatpak list --columns=ref` spells a runtime as
    # `<id>/<arch>/<branch>`, e.g. `org.gnome.Platform/x86_64/47`.
    for ref in org.gnome.Platform org.gnome.Sdk; do
        if ! flatpak list --runtime --columns=ref 2>/dev/null | grep -q "^${ref}/.*/47$"; then
            echo "error: the runtime the manifest pins is not installed: ${ref}//47" >&2
            echo "       run: flatpak install -y flathub org.gnome.Platform//47 org.gnome.Sdk//47" >&2
            exit 1
        fi
    done
fi

# The manifest resolves `path: files/...` next to itself, so the staging tree
# lives beside it. It is removed on the way out (and ignored by git).
files="$manifest_dir/files"
rm -rf "$files"
mkdir -p "$files"

install -m 755 "$binary" "$files/ownhub"
install -m 644 "$manifest_dir/$app_id.desktop" "$files/$app_id.desktop"
install -m 644 "$manifest_dir/$app_id.metainfo.xml" "$files/$app_id.metainfo.xml"
# Icon sizes the launcher and the AppStream entry reference. 256 and 512 use
# the 2x assets because the repo ships no dedicated size for them.
install -m 644 "$root/src-tauri/icons/32x32.png" "$files/32x32.png"
install -m 644 "$root/src-tauri/icons/64x64.png" "$files/64x64.png"
install -m 644 "$root/src-tauri/icons/128x128.png" "$files/128x128.png"
install -m 644 "$root/src-tauri/icons/128x128@2x.png" "$files/256x256.png"
install -m 644 "$root/src-tauri/icons/icon.png" "$files/512x512.png"

if [ "$stage_only" = 1 ]; then
    echo "== staged $app_id $version in $files"
    exit 0
fi

# Staging goes to the project's scratch root, never /tmp.
work=$(mktemp -d "${TMPDIR:-/var/tmp}/ownhub-flatpak.XXXXXXXXXX")
cleanup() {
    status=$?
    rm -rf "$work" 2>/dev/null || true
    rm -rf "$manifest_dir/files" 2>/dev/null || true
    exit "$status"
}
trap cleanup EXIT INT TERM

echo "== building $app_id $version from $binary"
# `--user` needs no root; `--disable-rofiles-fuse` drops the FUSE requirement,
# which is what breaks unattended builds inside containers.
flatpak-builder \
    --user \
    --force-clean \
    --disable-rofiles-fuse \
    --repo "$work/repo" \
    "$work/build" "$manifest"

rm -f "$output"
flatpak build-bundle "$work/repo" "$output" "$app_id"
echo "== wrote $output"
