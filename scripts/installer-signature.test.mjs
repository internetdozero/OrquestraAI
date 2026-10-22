import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { after, test } from "node:test";
import { createHash } from "node:crypto";

const scratchRoot = "/var/tmp/testing";
mkdirSync(scratchRoot, { recursive: true });
const fixtureRoot = mkdtempSync(join(scratchRoot, "orquestra-installer-signature-"));
const fixtureDir = join(fixtureRoot, "fixtures");
const binDir = join(fixtureRoot, "bin");
const homeDir = join(fixtureRoot, "home");
const tempDir = join(fixtureRoot, "tmp");
const assetName = "orquestra-linux-x86_64";
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
  */download/SHA256SUMS.txt)
    cp "$FIXTURE_DIR/SHA256SUMS.txt" "$output"
    printf '%s' 'https://github.com/internetdozero/OrquestraAI/releases/download/v1.2.3/SHA256SUMS.txt'
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

function runInstaller(args = []) {
  return spawnSync("sh", [installerPath, ...args], {
    encoding: "utf8",
    env: {
      ...process.env,
      FIXTURE_DIR: fixtureDir,
      HOME: homeDir,
      PATH: `${binDir}:${process.env.PATH}`,
      TMPDIR: tempDir,
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
  assert.equal(readFileSync(join(homeDir, ".local/bin/orquestra"), "utf8"), "fixture binary\n");
});

test("installer allows unsigned legacy releases only with explicit opt-in", () => {
  rmSync(signaturePath, { force: true });
  const result = runInstaller(["--allow-unsigned-legacy"]);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /explicitly allowing an unsigned legacy release/);
});

after(() => {
  rmSync(fixtureRoot, { recursive: true, force: true });
});
