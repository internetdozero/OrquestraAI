# Release Supply-Chain Security

Every OrquestraAI release signs its published `SHA256SUMS.txt` with an
offline Ed25519 release key and uploads the detached Minisign signature as
`SHA256SUMS.txt.minisig` next to it (see `scripts/sign-release.py` and the
`Sign SHA256SUMS.txt` step in `.github/workflows/build.yml`). Verifying
that signature _before_ trusting any SHA-256 checksum closes the
trust-on-first-use gap: a compromised release page serving a forged binary
with matching forged checksums still fails closed, because no valid
signature can be produced without the secret key.

## Release signing key

Public half, minisign `.pub` format (embedded in both `install.sh` as
`RELEASE_PUBKEY` and `src-tauri/src/update_sig.rs` as
`RELEASE_PUBLIC_KEY` — the two must always match):

```text
untrusted comment: orquestra release signing key
RWRXcQWxu51A3hiNwiOi7zV6Dm+uUXR4zZrg1IbsZqD8RAqHZwgKbeU1
```

The secret half lives only in the maintainer's offline copy and the
`UPDATE_SIGNING_SEED` CI secret — never in this repo. Releases predating
`v0.1.0-alpha.4` carry no `.minisig` asset and cannot be signature-checked.

## What install.sh enforces

1. Downloads the platform binary, `SHA256SUMS.txt`, and
   `SHA256SUMS.txt.minisig` into a `0700` staging dir.
2. Verifies the detached signature over the exact `SHA256SUMS.txt` bytes —
   reference `minisign` when present, otherwise an inline PyNaCl
   prehashed check (same math as the signer and the in-app updater).
3. Only then extracts the asset's SHA-256 entry and checks the binary.

A bad signature always fails closed. When no verifier (or no `.minisig`
asset) is available, the installer warns loudly and proceeds on SHA-256
alone — unless `--require-sig` is passed, which turns that into a hard
failure:

```sh
curl -fsSL https://orquestra.internetdozero.com.br/install.sh | bash -s -- --require-sig
```

## Manual verification

With the reference tool (any release with a `.minisig` asset):

```sh
minisign -Vm SHA256SUMS.txt -P RWRXcQWxu51A3hiNwiOi7zV6Dm+uUXR4zZrg1IbsZqD8RAqHZwgKbeU1 \
  -x SHA256SUMS.txt.minisig
```

Without minisign, with Python + PyNaCl (`pip install pynacl`):

```sh
python3 - RWRXcQWxu51A3hiNwiOi7zV6Dm+uUXR4zZrg1IbsZqD8RAqHZwgKbeU1 \
  SHA256SUMS.txt SHA256SUMS.txt.minisig <<'EOF'
import base64, hashlib, sys
from nacl.signing import VerifyKey
from nacl.exceptions import BadSignatureError
pub = base64.b64decode(sys.argv[1])
sums = open(sys.argv[2], 'rb').read()
sig = base64.b64decode(open(sys.argv[3]).read().splitlines()[1].strip())
assert sig[2:10] == pub[2:10], "signing key id mismatch"
digest = hashlib.blake2b(sums, digest_size=64).digest()
try:
    VerifyKey(pub[10:42]).verify(sig[10:74] + digest)
except BadSignatureError:
    sys.exit("INVALID signature")
print("signature valid")
EOF
```

Both accept only stock Minisign prehashed-mode signatures — the same gate
the auto-updater applies in `src-tauri/src/update_sig.rs` before it trusts
any downloaded checksum.
