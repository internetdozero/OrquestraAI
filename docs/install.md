# Installing OrquestraAI

The supported channel today is the installer served at
`https://orquestra.internetdozero.com.br/install.sh`, which verifies a detached
signature over the release checksums before it trusts anything (see
[security.md](security.md)) and installs the binary to `~/.local/bin/orquestra`.

```sh
curl -fsSL https://orquestra.internetdozero.com.br/install.sh | sh
```

## Native package (Debian/Ubuntu)

The Linux release also publishes a `.deb`, built from the same bundle metadata
as the installer path. It is the discoverable channel: `dpkg -i` puts the
binary in `/usr/bin`, registers the launcher and lets the package manager
record what is installed. It does not change the runtime requirements below —
the package links the same glibc and WebKitGTK.

```sh
curl -fLO https://github.com/internetdozero/OrquestraAI/releases/latest/download/<name>.deb
sudo dpkg -i <name>.deb
```

In-app updates replace the executable the app is running from. For a
package-managed install that is `/usr/bin/orquestra`, which needs write access;
the updater reports the permission error instead of failing silently, and
reinstalling the newer `.deb` is the intended path there.

## Linux distributions

The release is built on Ubuntu 22.04, which means the binary is linked against
glibc 2.35 and WebKitGTK 4.1. The loader is the hard limit: a distribution with
an older glibc stops before `main` with

```
/app/orquestra: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.34' not found
```

and no package install fixes that.

| Distribution                          | glibc | Runs the release as published |
| ------------------------------------- | ----- | ----------------------------- |
| Ubuntu 22.04+ / Debian 12+            | 2.35+ | yes                           |
| Fedora 36+, Arch, openSUSE Tumbleweed | 2.35+ | yes                           |
| Debian 11, Ubuntu 20.04               | 2.31  | no (`GLIBC_2.34`)             |
| RHEL / Rocky / Alma 8                 | 2.28  | no (`GLIBC_2.34`)             |
| Amazon Linux 2                        | 2.26  | no (`GLIBC_2.34`)             |

The installer reads the binary's own versioned symbols (`objdump -T`) and
compares them with `ldd --version` before replacing an installed binary, so the
failure is reported as a version mismatch instead of a loader crash. The same
check runs in CI against the published artifact
(`scripts/check-glibc.sh`, job `linux-compat`), so a regression shows up in the
build, not in a user's terminal.

### Running on an older distribution

Two paths exist, and neither is a package you can install today:

- **Flatpak** carries its own runtime, so it is not tied to the host glibc.
  Packaging is tracked as task #34; until it ships, there is no Flatpak build.
- **Build from source** on the target distribution (`cargo build --release`
  with that distribution's WebKitGTK development package) produces a binary
  linked against its own glibc, which is what an operator on Debian 11 needs
  right now.

An AppImage is not a solution for this class of failure: an AppImage still
links the host glibc, so it fails the same way.

## macOS

Two assets are published per architecture, and the installer takes the first
one that exists:

- `OrquestraAI-macos-<arch>.app.zip`: the app bundle. When the release was
  signed and notarized (the `APPLE_*` secrets were set for that tag), it opens
  from Finder without the "unidentified developer" dialog. The installer
  unpacks it into `~/Applications` and links `~/.local/bin/orquestra` at the
  executable inside the bundle, so the CLI and the updater keep working against
  the same Mach-O.
- `orquestra-macos-<arch>`: the bare binary. This is the shape older releases
  published, and it is still the updater's asset; without a bundle the app
  wraps itself in `~/Applications/OrquestraAI.app` on first launch (see
  [ARCHITECTURE.md](ARCHITECTURE.md)).

A build without the signing secrets is quarantined by macOS. Allow it once in
System Settings → Privacy & Security, right-click → Open, or clear the flag:

```sh
xattr -dr com.apple.quarantine ~/Applications/OrquestraAI.app
```

The bundle is built with the hardened runtime plus the JIT entitlement the
WebView needs (`src-tauri/entitlements.plist`); a hardened bundle without it
renders an empty window.

## RTK (optional tool)

The app can install [RTK](https://github.com/rtk-ai/rtk) for you, into
`~/.local/bin` on Linux and macOS. That directory is on `PATH` on most Linux
desktops, but not in zsh on macOS, so the success message names the exact line
to add when the directory is missing from `PATH`:

```sh
export PATH="$HOME/.local/bin:$PATH"  # add to ~/.zshrc or ~/.profile
```

On Windows the tool goes to `%LOCALAPPDATA%\OrquestraAI\bin`, and the message
prints the `setx PATH` command that adds it permanently.

## Windows

Not published yet: the Tauri shell still contains Unix-only code, so there is
no MSI or NSIS installer to install. Task #20 tracks that work, and until it
lands `install.sh` refuses Windows (it has no PowerShell counterpart yet).
