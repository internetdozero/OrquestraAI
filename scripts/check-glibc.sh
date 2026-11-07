#!/usr/bin/env bash
# Does this Linux binary run on the glibc installed here?
#
# The release is built on Ubuntu 22.04, so its dynamic symbols reference glibc
# 2.35; Debian 11 (2.31), Ubuntu 20.04 (2.31), RHEL/Rocky 8 (2.28) and Amazon
# Linux 2 (2.26) stop the loader with `GLIBC_2.34 not found`. `ldd` lists
# missing libraries but says nothing about symbol versions, which is why this
# check reads the binary's own versioned references instead.
#
# Usage: check-glibc.sh <binary>
#   0  this system can run it
#   1  it needs a newer glibc than this system has
#   2  the requirement could not be determined
#
# The same algorithm is inline in install.sh, which must stay a single
# self-contained stream for `curl | sh`. Change both together.
set -euo pipefail

binary=${1:?usage: check-glibc.sh <binary>}

if [ ! -f "$binary" ]; then
    echo "error: $binary does not exist" >&2
    exit 2
fi

reader=""
for candidate in objdump readelf; do
    if command -v "$candidate" >/dev/null 2>&1; then
        reader=$candidate
        break
    fi
done
if [ -z "$reader" ]; then
    echo "error: neither objdump nor readelf is installed (binutils)" >&2
    exit 2
fi

if [ "$reader" = "objdump" ]; then
    versions=$(objdump -T "$binary" 2>/dev/null || true)
else
    versions=$(readelf --dyn-syms --wide "$binary" 2>/dev/null || true)
fi
required=$(printf '%s\n' "$versions" \
    | grep -o 'GLIBC_[0-9][0-9.]*' \
    | sed 's/^GLIBC_//' \
    | sort -u -V || true)
if [ -z "$required" ]; then
    echo "error: $binary references no versioned glibc symbols; is it a Linux binary?" >&2
    exit 2
fi

if ! available=$(ldd --version 2>/dev/null | head -1 | grep -o '[0-9][0-9.]*$' || true); then
    available=""
fi
if [ -z "$available" ]; then
    echo "error: could not read the glibc version (ldd --version)" >&2
    exit 2
fi

newest=$(printf '%s\n' "$required" | tail -1)
echo "binary needs glibc >= ${newest}; this system has ${available}"

if [ "$newest" = "$available" ] || [ "$(printf '%s\n%s\n' "$newest" "$available" | sort -V | tail -1)" = "$available" ]; then
    exit 0
fi

echo "error: ${binary} requires glibc ${newest}, but this system provides ${available}." >&2
echo "       The loader will fail with 'GLIBC_${newest} not found'." >&2
echo "       Use a distribution with glibc >= ${newest}, or the Flatpak build," >&2
echo "       which carries its own runtime (see docs/install.md)." >&2
exit 1
