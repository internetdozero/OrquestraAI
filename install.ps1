<#
.SYNOPSIS
    OwnHub installer for Windows.

.DESCRIPTION
    The PowerShell counterpart of install.sh, with the same trust model: the
    release's SHA256SUMS.txt carries a detached Ed25519 signature from the
    offline release key, that signature is verified BEFORE any checksum is
    trusted, and the downloaded executable must match its SHA-256 entry.

    Signature verification needs `minisign` (winget install jedisct1.minisign).
    Unlike install.sh there is no PyNaCl fallback: requiring one verifier and
    naming how to get it is better than shipping a second, less-tested
    verification path.

.PARAMETER Version
    Release tag to install (e.g. v0.1.0-alpha.7). Defaults to the latest.

.PARAMETER InstallDir
    Destination directory. Defaults to
    %LOCALAPPDATA%\Programs\OwnHub, which needs no elevation.

.PARAMETER CheckDeps
    Only verify the installed executable; do not download anything.

.PARAMETER NoPathUpdate
    Do not add the install directory to the user PATH.

.EXAMPLE
    irm https://ownhub.internetdozero.com.br/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -CheckDeps
#>
[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\OwnHub'),
    [switch]$CheckDeps,
    [switch]$NoPathUpdate,
    [switch]$TestOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Repo = 'OwnAnAI/ownhub'
# Must equal src-tauri/src/update_sig.rs::RELEASE_PUBLIC_KEY and the value
# embedded in install.sh: the same offline key signs every release.
$ReleasePublicKey = 'RWRXcQWxu51A3hiNwiOi7zV6Dm+uUXR4zZrg1IbsZqD8RAqHZwgKbeU1'
$Asset = 'ownhub-windows-x86_64.exe'
$BinaryName = 'ownhub.exe'

function Write-Step([string]$Message) { Write-Host "==> $Message" }
function Write-Ok([string]$Message) { Write-Host "OK  $Message" -ForegroundColor Green }

function Get-Architecture {
    # A 32-bit PowerShell on 64-bit Windows reports x86 and hides the host in
    # PROCESSOR_ARCHITEW6432; prefer the real machine architecture.
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    if ($arch -eq 'ARM64') {
        throw 'No Windows ARM64 build is published yet; use an x64 install.'
    }
    if ($arch -ne 'AMD64' -and $arch -ne 'x86') {
        throw "Unsupported processor architecture: $arch"
    }
    return 'x86_64'
}

function Resolve-ReleaseTag([string]$Requested) {
    if ($Requested) {
        if ($Requested -notmatch '^[A-Za-z0-9._+-]+$') {
            throw "Invalid release tag: $Requested"
        }
        return $Requested
    }
    # /releases/latest redirects to /releases/tag/<tag>. Reading the redirect
    # costs no API quota and cannot race a moving "latest". Windows PowerShell
    # throws on the 3xx when redirection is capped while PowerShell 7 returns
    # it, so both shapes are read.
    $location = $null
    try {
        $response = Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" `
            -MaximumRedirection 0 -UseBasicParsing -ErrorAction Stop
        $location = $response.Headers['Location']
    }
    catch {
        $response = $_.Exception.Response
        if ($response -and $response.Headers) { $location = $response.Headers['Location'] }
    }
    if ($location -is [array]) { $location = $location[0] }
    if ($location -and ([string]$location) -match '/releases/tag/([^/?#]+)') { return $Matches[1] }
    throw 'Could not resolve the latest release tag.'
}

function Save-File([string]$Uri, [string]$Path) {
    Invoke-WebRequest -Uri $Uri -OutFile $Path -UseBasicParsing
}

function Assert-Signature([string]$SumsPath, [string]$SigPath, [string]$ExpectedVersion) {
    $minisign = Get-Command minisign -ErrorAction SilentlyContinue
    if (-not $minisign) {
        throw @'
Release signature verification needs 'minisign':
    winget install jedisct1.minisign
The installer stops rather than skip signature verification.
'@
    }
    # -V verifies the detached signature; -P pins the key, so a release page
    # that swaps both the binary and the signature still fails.
    $output = & $minisign.Source -V -P $ReleasePublicKey -m $SumsPath -x $SigPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Release signature INVALID: $output"
    }
    Assert-TrustedComment -SignaturePath $SigPath -ExpectedVersion $ExpectedVersion
    Write-Ok 'Release signature and version binding verified.'
}

function Assert-TrustedComment([string]$SignaturePath, [string]$ExpectedVersion) {
    # minisign's trusted comment is authenticated by its global signature, but
    # the CLI does not print its fields. Parse the exact four-line format the
    # release signer emits; Assert-Signature calls `minisign -V` before this,
    # which verifies both signatures before the version binding is trusted.
    $signatureLines = @(Get-Content -Path $SignaturePath)
    if ($signatureLines.Count -ne 4) { throw 'The release signature metadata is malformed.' }
    $commentMatch = [regex]::Match($signatureLines[2], '^trusted comment: (.+)$')
    if (-not $commentMatch.Success) { throw 'The release signature metadata is malformed.' }

    $fields = @{}
    foreach ($field in $commentMatch.Groups[1].Value -split "`t") {
        $pair = [regex]::Match($field, '^([^=]+)=(.*)$')
        if (-not $pair.Success) { throw 'The trusted comment contains a malformed field.' }
        $name = $pair.Groups[1].Value
        if ($fields.ContainsKey($name)) { throw "Duplicate trusted-comment field: $name" }
        $fields[$name] = $pair.Groups[2].Value
    }
    if ($fields.Count -ne 3 -or $fields['timestamp'] -notmatch '^\d+$' -or
        $fields['file'] -ne 'SHA256SUMS.txt' -or $fields['version'] -ne $ExpectedVersion) {
        throw "The signed comment does not bind SHA256SUMS.txt to version $ExpectedVersion."
    }
}

function Get-ExpectedChecksum([string]$SumsPath, [string]$FileName) {
    $pattern = '^(?<hash>[A-Fa-f0-9]{64})\s+\*?' + [regex]::Escape($FileName) + '$'
    $hits = @(Get-Content -Path $SumsPath | Where-Object { $_ -match $pattern })
    if ($hits.Count -eq 0) { throw "No SHA-256 entry for $FileName in SHA256SUMS.txt." }
    # More than one entry means the file cannot be pinned: a release that
    # lists the same asset twice is either broken or forged.
    if ($hits.Count -gt 1) { throw "Ambiguous SHA-256 entries for $FileName." }
    return ([regex]::Match($hits[0], $pattern)).Groups['hash'].Value.ToLowerInvariant()
}

function Test-Installed {
    $binary = Join-Path $InstallDir $BinaryName
    if (-not (Test-Path $binary)) {
        throw "OwnHub is not installed at $binary"
    }
    # There is no `--version` flag: an unknown argument starts the GUI. A
    # documented headless flag proves the executable runs without opening a
    # window, and a non-zero exit still means the binary itself executed.
    Write-Step "Smoke-testing $binary --bridge-status"
    $output = & $binary --bridge-status 2>&1
    $first = @($output) | Select-Object -First 3
    if ($first.Count -gt 0) { Write-Host (($first | Out-String).Trim()) }
    Write-Ok "Installed at $binary (exit $LASTEXITCODE)"
}

function Add-ToUserPath([string]$Directory) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @()
    if ($userPath) { $entries = $userPath -split ';' | Where-Object { $_ } }
    if ($entries -contains $Directory) {
        Write-Ok "$Directory is already on the user PATH."
        return
    }
    $updated = (@($entries) + $Directory) -join ';'
    [Environment]::SetEnvironmentVariable('Path', $updated, 'User')
    $env:Path = "$env:Path;$Directory"
    Write-Ok "Added $Directory to the user PATH (open a new terminal to see it)."
}

if ($TestOnly) {
    return
}

if ($CheckDeps) {
    Test-Installed
    return
}

$architecture = Get-Architecture
Write-Step "Platform: windows/$architecture"
$tag = Resolve-ReleaseTag $Version
$releaseVersion = $tag.TrimStart('v')
$base = "https://github.com/$Repo/releases/download/$tag"
Write-Step "Release: $tag"

$staging = Join-Path ([System.IO.Path]::GetTempPath()) ("ownhub-install-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $staging -Force | Out-Null
try {
    $sums = Join-Path $staging 'SHA256SUMS.txt'
    $sig = Join-Path $staging 'SHA256SUMS.txt.minisig'
    $download = Join-Path $staging $Asset

    Write-Step 'Downloading checksums, signature and executable'
    Save-File "$base/SHA256SUMS.txt" $sums
    Save-File "$base/SHA256SUMS.txt.minisig" $sig
    Save-File "$base/$Asset" $download

    Assert-Signature $sums $sig $releaseVersion

    Write-Step 'Verifying the executable SHA-256'
    $expected = Get-ExpectedChecksum $sums $Asset
    $actual = (Get-FileHash -Path $download -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        throw "SHA-256 mismatch: expected $expected, got $actual"
    }
    Write-Ok 'Executable matches the signed checksum.'

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $target = Join-Path $InstallDir $BinaryName
    # Stage beside the target, then move: a failed copy must not leave a
    # half-written executable where the old one was.
    $staged = Join-Path $InstallDir ('.ownhub-install-' + [guid]::NewGuid().ToString('N'))
    Copy-Item -Path $download -Destination $staged -Force
    Unblock-File -Path $staged -ErrorAction SilentlyContinue
    Move-Item -Path $staged -Destination $target -Force
    Write-Ok "Installed to $target"

    if (-not $NoPathUpdate) { Add-ToUserPath $InstallDir }

    Write-Host ''
    Write-Ok "OwnHub $releaseVersion is installed."
    Write-Host 'Run "ownhub" in a new terminal to start the control plane.'
}
finally {
    Remove-Item -Path $staging -Recurse -Force -ErrorAction SilentlyContinue
}
