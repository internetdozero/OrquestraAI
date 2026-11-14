import assert from "node:assert/strict";
import os from "node:os";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { after, test } from "node:test";
import { createHash } from "node:crypto";

const scratchRoot = process.env.OWNHUB_TEST_TMP || os.tmpdir();
mkdirSync(scratchRoot, { recursive: true });
const fixtureRoot = mkdtempSync(join(scratchRoot, "ownhub-installer-signature-"));
const fixtureDir = join(fixtureRoot, "fixtures");
const binDir = join(fixtureRoot, "bin");
const homeDir = join(fixtureRoot, "home");
const tempDir = join(fixtureRoot, "tmp");
const assetName = "ownhub-linux-x86_64";
const assetPath = join(fixtureDir, assetName);
const sumsPath = join(fixtureDir, "SHA256SUMS.txt");
const signaturePath = join(fixtureDir, "SHA256SUMS.txt.minisig");
const installerPath = resolve("install.sh");

mkdirSync(fixtureDir, { recursive: true });
mkdirSync(binDir, { recursive: true });
mkdirSync(homeDir, { recursive: true });
mkdirSync(tempDir, { recursive: true });
writeFileSync(assetPath, "fixture binary\n");
const checksum = createHash("sha256").update("fixture binary\n").digest("hex");
writeFileSync(sumsPath, `${checksum}  ${assetName}\n`);

writeFileSync(
  join(binDir, "curl"),
  `#!/bin/sh
set -e
output=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) output="$2"; shift 2 ;;
    -w) shift 2 ;;
    -*) shift ;;
    *) url="$1"; shift ;;
  esac
done
case "$url" in
  */releases/latest)
    printf '%s' 'https://github.com/internetdozero/OwnHub/releases/tag/v1.2.3'
    ;;
  */releases/download/v1.2.3/SHA256SUMS.txt)
    cp "$FIXTURE_DIR/SHA256SUMS.txt" "$output"
    ;;
  */SHA256SUMS.txt.minisig)
    if [ -s "$FIXTURE_DIR/SHA256SUMS.txt.minisig" ]; then
      cp "$FIXTURE_DIR/SHA256SUMS.txt.minisig" "$output"
    else
      exit 22
    fi
    ;;
  */${assetName}) cp "$FIXTURE_DIR/${assetName}" "$output" ;;
  */src-tauri/icons/icon.png) printf 'icon' > "$output" ;;
  *) echo "unexpected URL: $url" >&2; exit 22 ;;
esac
`,
);
chmodSync(join(binDir, "curl"), 0o755);

writeFileSync(join(binDir, "minisign"), "#!/bin/sh\nexit 0\n");
chmodSync(join(binDir, "minisign"), 0o755);

// `objdump -T` is what the installer reads the binary's versioned glibc
// symbols from. Text fixtures have none, so without this stub every run would
// skip the check; `OLD_GLIBC` makes it report a requirement the fake `ldd`
// below cannot satisfy.
writeFileSync(
  join(binDir, "objdump"),
  `#!/bin/sh
if [ "$OLD_GLIBC" = "1" ]; then
  printf '%s\\n' '0000000000000000  w   DF *UND*\t0000000000000000  GLIBC_2.34  pthread_create'
fi
exit 0
`,
);
chmodSync(join(binDir, "objdump"), 0o755);

writeFileSync(
  join(binDir, "ldd"),
  `#!/bin/sh
if [ "$1" = "--version" ]; then
  if [ "$OLD_GLIBC" = "1" ]; then
    printf '%s\\n' 'ldd (GNU libc) 2.31'
  else
    printf '%s\\n' 'ldd (GNU libc) 2.35'
  fi
  exit 0
fi
if [ "$LDD_MODE" = "missing" ]; then
  printf '%s\\n' 'libwebkit2gtk-4.1.so.0 => not found' 'libgtk-3.so.0 => not found'
elif [ "$LDD_MODE" = "auto-install" ]; then
  if [ -f "$FIXTURE_DIR/deps-installed" ]; then
    printf '%s\\n' 'libc.so.6 => /lib/libc.so.6 (0x00000000)'
  else
    printf '%s\\n' 'libwebkit2gtk-4.1.so.0 => not found' 'libgtk-3.so.0 => not found'
  fi
else
  printf '%s\\n' 'libc.so.6 => /lib/libc.so.6 (0x00000000)'
fi
`,
);
chmodSync(join(binDir, "ldd"), 0o755);

writeFileSync(
  join(binDir, "pacman"),
  `#!/bin/sh
if [ "$1" = "-S" ] || [ "$2" = "-S" ]; then
  touch "$FIXTURE_DIR/deps-installed"
  exit 0
fi
exit 0
`,
);
chmodSync(join(binDir, "pacman"), 0o755);

writeFileSync(
  join(binDir, "sudo"),
  `#!/bin/sh
"$@"
`,
);
chmodSync(join(binDir, "sudo"), 0o755);

function runInstaller(args = [], extraEnv = {}) {
  return spawnSync("sh", [installerPath, ...args], {
    encoding: "utf8",
    env: {
      ...process.env,
      FIXTURE_DIR: fixtureDir,
      HOME: homeDir,
      PATH: `${binDir}:${process.env.PATH}`,
      TMPDIR: tempDir,
      ...extraEnv,
    },
  });
}

test("installer requires signatures and binds them to the selected tag", () => {
  let result = runInstaller();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /release signature is missing/);

  writeFileSync(
    signaturePath,
    "untrusted comment: test\nAAAA\ntrusted comment: timestamp=123\tfile=SHA256SUMS.txt\tversion=9.9.9\nAAAA\n",
  );
  result = runInstaller();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /signature metadata does not match this release/);

  writeFileSync(
    signaturePath,
    "untrusted comment: test\nAAAA\ntrusted comment: timestamp=123\tfile=SHA256SUMS.txt\tversion=1.2.3\nAAAA\n",
  );
  result = runInstaller();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(homeDir, ".local/bin/ownhub"), "utf8"), "fixture binary\n");
});

test("installer allows unsigned legacy releases only with explicit opt-in", () => {
  rmSync(signaturePath, { force: true });
  const result = runInstaller(["--allow-unsigned-legacy"]);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /explicitly allowing an unsigned legacy release/);
});

test("runtime dependencies can be checked after installation without downloading again", () => {
  const installedBinary = join(homeDir, ".local/bin/ownhub");
  writeFileSync(installedBinary, "existing installation\n");
  chmodSync(installedBinary, 0o755);

  const result = runInstaller(["--check-deps"], { LDD_MODE: "missing" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /libwebkit2gtk-4\.1\.so\.0/);
  assert.match(result.stderr, /libgtk-3\.so\.0/);
  assert.equal(readFileSync(installedBinary, "utf8"), "existing installation\n");
});

test("installer refuses a binary this glibc cannot load and names the Flatpak", () => {
  const installedBinary = join(homeDir, ".local/bin/ownhub");
  writeFileSync(installedBinary, "existing installation\n");
  chmodSync(installedBinary, 0o755);

  const result = runInstaller(["--check-deps"], { OLD_GLIBC: "1" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /requires glibc 2\.34/);
  // The loader cannot be fixed by installing a package, so the message has to
  // name the channel that does work. It is the same text CI asserts on the
  // debian:11 and rockylinux:8 jobs.
  assert.match(result.stderr, /flatpak install --user/);
  assert.match(result.stderr, /OwnHub-x86_64\.flatpak/);
  assert.equal(readFileSync(installedBinary, "utf8"), "existing installation\n");
});

test("installer refuses to replace the current binary when runtime libraries are missing", () => {
  writeFileSync(
    signaturePath,
    "untrusted comment: test\nAAAA\ntrusted comment: timestamp=123\tfile=SHA256SUMS.txt\tversion=1.2.3\nAAAA\n",
  );
  const installedBinary = join(homeDir, ".local/bin/ownhub");
  writeFileSync(installedBinary, "existing installation\n");
  chmodSync(installedBinary, 0o755);

  const result = runInstaller([], { LDD_MODE: "missing" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Installation stopped before replacing/);
  assert.equal(readFileSync(installedBinary, "utf8"), "existing installation\n");
});

test("installer automatically installs missing runtime libraries when supported package manager is available", () => {
  rmSync(join(fixtureDir, "deps-installed"), { force: true });
  writeFileSync(
    signaturePath,
    "untrusted comment: test\nAAAA\ntrusted comment: timestamp=123\tfile=SHA256SUMS.txt\tversion=1.2.3\nAAAA\n",
  );
  const installedBinary = join(homeDir, ".local/bin/ownhub");
  writeFileSync(installedBinary, "old binary\n");
  chmodSync(installedBinary, 0o755);

  const result = runInstaller([], { LDD_MODE: "auto-install" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Auto-installing missing system dependencies/);
  assert.match(result.stdout, /webkit2gtk-4\.1/);
  assert.equal(readFileSync(installedBinary, "utf8"), "fixture binary\n");
});

after(() => {
  rmSync(fixtureRoot, { recursive: true, force: true });
});
