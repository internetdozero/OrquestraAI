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

1. Downloads `SHA256SUMS.txt` via the latest-release redirect and resolves
   its exact tag. It then downloads the binary and signature from that tag into
   a `0700` staging dir.
2. Verifies the detached signature and requires the trusted comment to name
   `SHA256SUMS.txt` and that exact release version — reference `minisign` when
   present, otherwise an inline PyNaCl prehashed check.
3. Only then extracts the asset's SHA-256 entry and checks the binary.

A bad, missing, or unverifiable signature fails closed by default. Releases
from before signing can still be installed with an explicit opt-in to
checksum-only verification:

```sh
curl -fsSL https://orquestra.internetdozero.com.br/install.sh | bash -s -- --allow-unsigned-legacy
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
  SHA256SUMS.txt SHA256SUMS.txt.minisig 0.1.0-alpha.5 <<'EOF'
import base64, hashlib, sys
from nacl.signing import VerifyKey
pub = base64.b64decode(sys.argv[1])
sums = open(sys.argv[2], 'rb').read()
expected_version = sys.argv[4]
lines = open(sys.argv[3]).read().splitlines()
assert len(lines) == 4 and lines[2].startswith('trusted comment: ')
sig = base64.b64decode(lines[1].strip(), validate=True)
trusted = lines[2][len('trusted comment: '):]
global_sig = base64.b64decode(lines[3].strip(), validate=True)
assert sig[:2] == b'ED' and sig[2:10] == pub[2:10], "signing key id mismatch"
fields = dict(field.split('=', 1) for field in trusted.split('\t') if '=' in field)
assert fields.get('file') == 'SHA256SUMS.txt' and fields.get('version') == expected_version
digest = hashlib.blake2b(sums, digest_size=64).digest()
key = VerifyKey(pub[10:42])
key.verify(digest, sig[10:74])
key.verify(sig[10:74] + trusted.encode(), global_sig)
print("signature valid")
EOF
```

Both accept only stock Minisign prehashed-mode signatures — the same gate
the auto-updater applies in `src-tauri/src/update_sig.rs` before it trusts
any downloaded checksum.
