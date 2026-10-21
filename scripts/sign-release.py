#!/usr/bin/env python3
"""Sign a release artifact with the offline Ed25519 release key.

Emits `<file>.minisig` in stock minisign prehashed mode, verifiable with the
reference `minisign -V` tool and with `src-tauri/src/update_sig.rs`.

Key material: env UPDATE_SIGNING_SEED holds `seedhex:keynumhex` (the same
content as the maintainer's offline `update-signing-seed.hex`). The public
half is embedded in the binary; the seed never appears in logs.

Usage: python3 scripts/sign-release.py release-assets/SHA256SUMS.txt
Requires: pip install pynacl
"""

import base64
import hashlib
import os
import sys
import time

from nacl.signing import SigningKey


def b64(raw: bytes) -> str:
    return base64.b64encode(raw).decode()


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <file-to-sign>", file=sys.stderr)
        return 2
    target = sys.argv[1]
    seed_spec = os.environ.get("UPDATE_SIGNING_SEED", "")
    try:
        seed_hex, keynum_hex = seed_spec.split(":")
        seed = bytes.fromhex(seed_hex)
        keynum = bytes.fromhex(keynum_hex)
        assert len(seed) == 32 and len(keynum) == 8
    except (ValueError, AssertionError):
        print("error: UPDATE_SIGNING_SEED must be `seedhex:keynumhex`", file=sys.stderr)
        return 1
    with open(target, "rb") as handle:
        data = handle.read()
    signing = SigningKey(seed)
    digest = hashlib.blake2b(data, digest_size=64).digest()
    sig = bytes(signing.sign(digest).signature)
    trusted_content = f"timestamp={int(time.time())}\tfile={os.path.basename(target)}"
    global_sig = bytes(signing.sign(sig + trusted_content.encode()).signature)
    minisig = (
        "untrusted comment: signature from orquestra release key\n"
        f"{b64(b'ED' + keynum + sig)}\n"
        f"trusted comment: {trusted_content}\n"
        f"{b64(global_sig)}"
    )
    out_path = target + ".minisig"
    with open(out_path, "w") as handle:
        handle.write(minisig)
    print(f"signed {target} -> {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
