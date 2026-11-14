# Installing OwnHub

The supported channel today is the installer served at
`https://ownhub.internetdozero.com.br/install.sh`, which verifies a detached
signature over the release checksums before it trusts anything (see
[security.md](security.md)) and installs the binary to `~/.local/bin/ownhub`.

```sh
curl -fsSL https://ownhub.internetdozero.com.br/install.sh | sh
```

## Native package (Debian/Ubuntu)

The Linux release also publishes a `.deb`, built from the same bundle metadata
as the installer path. It is the discoverable channel: `dpkg -i` puts the
binary in `/usr/bin`, registers the launcher and lets the package manager
record what is installed. It declares its Debian dependencies — WebKitGTK 4.1,
GTK 3, libsoup 3, libayatana-appindicator, librsvg and `xdg-utils` — so
`dpkg -i` resolves them instead of leaving a binary that cannot start. The
glibc requirement still comes from the binary itself, not from a package.

```sh
curl -fLO https://github.com/internetdozero/OwnHub/releases/latest/download/<name>.deb
sudo dpkg -i <name>.deb
```

In-app updates replace the executable the app is running from. For a
package-managed install that is `/usr/bin/ownhub`, which needs write access;
the updater reports the permission error instead of failing silently, and
reinstalling the newer `.deb` is the intended path there.

## Linux distributions

The release is built on Ubuntu 22.04, which means the binary is linked against
glibc 2.35 and WebKitGTK 4.1. The loader is the hard limit: a distribution with
an older glibc stops before `main` with

```
/app/ownhub: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.34' not found
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
failure is reported as a version mismatch with the Flatpak command to use
instead of a loader crash. The same check runs in CI against the published
artifact (`scripts/check-glibc.sh`, job `linux-compat`), which fails only when
the requirement cannot be measured — a distribution below the baseline is the
recorded, expected outcome there, not a regression.

### Running on an older distribution

- **Flatpak** is the supported channel there. The release publishes
  `OwnHub-x86_64.flatpak`, built from the same binary as every other
  channel (`.github/workflows/build.yml`, job `flatpak`) and wrapped by
  `packaging/flatpak/dev.ownanai.hub.yml`. It runs inside the GNOME 47
  runtime, which supplies glibc, GTK 3 and WebKitGTK 4.1 itself, so the host
  only needs a working `flatpak`:

  ```sh
  curl -fLO https://github.com/internetdozero/OwnHub/releases/latest/download/OwnHub-x86_64.flatpak
  flatpak install --user ./OwnHub-x86_64.flatpak
  flatpak run dev.ownanai.hub
  ```

  If the asset is missing from the latest release page, the packaging job
  failed: the binary and `.deb` channels are unaffected and the bundle returns
  with the next release.

  Two consequences of the sandbox are worth knowing before installing. The
  app manages user-level harness files (`~/.claude`, `~/.codex`, `~/.agents`,
  `~/.ownhub`), so the manifest declares `--filesystem=home`: a narrower
  share would leave every harness read-only. And in-app updates cannot replace
  a binary inside `/app`, so the updater reports a permission error and the
  Flatpak is updated by installing the newer bundle.

  The tray icon needs a StatusNotifier host. When the desktop has none,
  closing the window exits instead of leaving a headless process
  (`src-tauri/src/tray.rs`).

- **Build from source** on the target distribution (`cargo build --release`
  with that distribution's WebKitGTK development package) produces a binary
  linked against its own glibc.

An AppImage is not a solution for this class of failure: an AppImage still
links the host glibc, so it fails the same way.

### Flatpak sandbox limits

Only what the GNOME runtime ships is executable inside the sandbox. Host
binaries are not visible, even with `--filesystem=home`, because the loader and
`/usr` differ. `git`, `node`/`npx` and the Python `venv` module are therefore
unavailable in the Flatpak channel; `skills` sync/import and official MCP
install require the `.deb`, the installer binary or the AppImage. The app
reports them as missing instead of failing at the first click, and the
Settings → About probe names the sandbox.

The vault key uses `org.freedesktop.secrets`; the manifest declares
`--talk-name=org.freedesktop.secrets` and `--filesystem=xdg-run/keyring`. When
no provider answers, the app falls back to a `0600` key file and says so.

## Runtime tools (Linux)

The binary links its UI stack, but several features shell out to commands that
are not shared libraries. `install.sh` diagnoses and installs them unless
`--no-auto-deps` was passed; the `.deb` declares `xdg-utils` and the library
dependencies.

| Command                  | Feature                               | Package                            |
| ------------------------ | ------------------------------------- | ---------------------------------- |
| `xdg-open` or `gio`      | open links, reveal folders            | `xdg-utils` (or GLib's `gio`)      |
| `git`                    | skill import, official extension sync | `git`                              |
| `python3` + `venv`       | install official Python MCP servers   | `python3`, `python3-venv` (Debian) |
| `node` / `npx`           | start npx-based MCP presets           | `nodejs`/`npm`                     |
| `dbus` session + keyring | keep the vault key in the OS keyring  | `gnome-keyring` or `kwallet`       |

Without a keyring the vault still works, protected by a `0600` key file beside
the ciphertext; Settings → About shows the backend and the reason.

### Wayland and NVIDIA

WebKitGTK 2.42+ renders through DMA-BUF, which produces a blank window or a
crash with the proprietary NVIDIA driver on Wayland. Launch with

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 ownhub
# or, equivalently:
OWNHUB_WEBKIT_SOFTWARE=1 ownhub
```

To test the X11 path instead, set `GDK_BACKEND=x11`; the app never overrides an
explicit `GDK_BACKEND`.

## macOS

Two assets are published per architecture, and the installer takes the first
one that exists:

- `OwnHub-macos-<arch>.app.zip`: the app bundle. When the release was
  signed and notarized (the `APPLE_*` secrets were set for that tag), it opens
  from Finder without the "unidentified developer" dialog. The installer
  unpacks it into `~/Applications` and links `~/.local/bin/ownhub` at the
  executable inside the bundle, so the CLI and the updater keep working against
  the same Mach-O.
- `ownhub-macos-<arch>`: the bare binary. This is the shape older releases
  published, and it is still the updater's asset; without a bundle the app
  wraps itself in `~/Applications/OwnHub.app` on first launch (see
  [ARCHITECTURE.md](ARCHITECTURE.md)).

A build without the signing secrets is quarantined by macOS. Allow it once in
System Settings → Privacy & Security, right-click → Open, or clear the flag:

```sh
xattr -dr com.apple.quarantine ~/Applications/OwnHub.app
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

On Windows the tool goes to `%LOCALAPPDATA%\OwnHub\bin`, and the message
prints the `setx PATH` command that adds it permanently.

## Windows

The Windows build is published as an NSIS setup (`OwnHub_<version>_x64-setup.exe`)
and, next to it, the bare `ownhub-windows-x86_64.exe` that in-app updates
replace in place. The setup installs for the current user only, into
`%LOCALAPPDATA%\OwnHub`, so it needs no administrator rights and the
updater can overwrite the executable it is running from.

```powershell
$release = 'https://github.com/internetdozero/OwnHub/releases/latest/download'
Start-Process "$release/OwnHub_<version>_x64-setup.exe"
```

The script channel is `install.ps1`, the counterpart of `install.sh`, with the
same trust model: the detached signature over `SHA256SUMS.txt` is verified
before any checksum is trusted, and the downloaded executable must match its
signed SHA-256 entry.

```powershell
irm https://ownhub.internetdozero.com.br/install.ps1 | iex
```

It installs `ownhub.exe` (the same command name the Unix installer uses)
into `%LOCALAPPDATA%\Programs\OwnHub` and adds that directory to the user
`PATH`; `-InstallDir` changes the destination and `-NoPathUpdate` skips the
`PATH` edit. Unlike the setup, the script keeps its own directory, so the two
channels never share files.

Signature verification needs the reference `minisign` tool:

```powershell
winget install jedisct1.minisign
```

There is deliberately no second verification path: the installer stops with an
explanation instead of skipping the check when `minisign` is missing.
`install.ps1 -CheckDeps` smoke-tests an existing install without downloading
anything, and `scripts/Test-Install.ps1` covers the parsing rules offline (CI
runs it on `windows-latest`).

Windows ARM64 is not published; `install.ps1` refuses it instead of installing
an x64 build that would run under emulation.

`install.sh` still refuses Windows: run the setup or `install.ps1` there.
