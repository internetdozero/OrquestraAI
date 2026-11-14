# Release Supply-Chain Security

Every OwnHub release signs its published `SHA256SUMS.txt` with an
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
untrusted comment: ownhub release signing key
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
curl -fsSL https://ownhub.internetdozero.com.br/install.sh | bash -s -- --allow-unsigned-legacy
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

## Backup archive privacy

`~/.ownhub/backups/*.tar.gz` embeds `vault.json` with cleartext service
keys, so the archive is treated as a secret at rest:

- The file is created with mode `0600` **before** the first byte is written,
  and the mode is re-applied after closing. Without that, the inherited umask
  (`0644` on most systems) leaves the archive readable by every local user for
  as long as it takes to fill it.
- `~/.ownhub/backups/` is tightened to `0700`, so the names inside are not
  listable either.
- Archiving happens in-process (`tar` + `flate2`), never through an external
  `tar` binary, so no child process can be selected from `PATH` and no
  platform flag dialect is involved.
- Symlinks are stored as links, matching the previous `tar` invocation: the
  projections do not duplicate the vault into the archive.

On Windows there are no POSIX mode bits. The archive is created inside the
operator's profile (`%USERPROFILE%\.ownhub\backups`), which inherits the
profile ACL: the owning account, `SYSTEM` and the local `Administrators` group
— no ordinary local user. That is the guarantee the code relies on; it is not
a per-file DACL, so a profile directory whose ACL was widened by hand widens
the backups with it. Deployments that need a stronger guarantee should point
`USERPROFILE` at a directory with a restrictive ACL.

## Runtime tool resolution

The app shells out to a small, fixed set of programs (`git`, `python3`,
`node`, a desktop opener) and the vault's blind `vault exec` proxy shells out
to its own allowlist. Both use the same resolver
(`ownhub_core::exec`): the process `PATH` first, then the login shell's
`PATH`, `~/.local/bin`, `~/.cargo/bin`, mise/asdf shims and Homebrew; on
Windows the user `PATH` comes from `HKCU\Environment`.

- The login shell (`$SHELL -lc`) runs once per process, with a two-second
  deadline, and only when a plain `PATH` lookup already failed.
- `vault exec` validates the bare allowlisted name and the per-binary
  subcommand policy **before** resolving it. IPC cannot supply a path, and a
  modified rc file can only choose among executables that already carry an
  allowed name; the allowlist itself is still code-reviewed and has no
  runtime knob.
- Nothing is installed silently: a missing program is reported with the
  package name for the distribution, and the Flatpak sandbox says plainly
  that host binaries are not visible.
