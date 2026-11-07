$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot '..\install.ps1') -TestOnly

# Multi-line fixtures cannot be written with `Set-Content -Value @(...)` (Windows
# PowerShell writes one line per item, PowerShell 7 joins them with spaces) nor
# as a multi-line `@( 'a' + 'x', 'b' + 'y' )` literal — the `+` binds to the next
# element, and the array collapses into one space-joined string. Build the array
# with `+=` and write it through `WriteAllLines`, which writes one line per
# element on every version.
function Write-Lines([string]$Path, [string[]]$Lines) {
    [System.IO.File]::WriteAllLines($Path, $Lines)
}

$root = Join-Path ([System.IO.Path]::GetTempPath()) ("orquestra-install-test-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root -Force | Out-Null
try {
    $sums = Join-Path $root 'SHA256SUMS.txt'
    $sig = Join-Path $root 'SHA256SUMS.txt.minisig'

    $assetHash = 'a' * 64
    $otherHash = 'b' * 64
    $duplicateHash = 'c' * 64
    $sumLines = @()
    $sumLines += "$assetHash  orquestra-windows-x86_64.exe"
    $sumLines += "$otherHash  SHA256SUMS.txt"
    Write-Lines -Path $sums -Lines $sumLines

    $actual = Get-ExpectedChecksum $sums 'orquestra-windows-x86_64.exe'
    if ($actual -ne $assetHash) { throw 'checksum parser returned the wrong asset hash' }

    # A release that lists the same asset twice cannot be pinned.
    $sumLines += "$duplicateHash  orquestra-windows-x86_64.exe"
    Write-Lines -Path $sums -Lines $sumLines
    $ambiguous = $false
    try { Get-ExpectedChecksum $sums 'orquestra-windows-x86_64.exe' | Out-Null }
    catch { $ambiguous = $true }
    if (-not $ambiguous) { throw 'checksum parser accepted duplicate asset entries' }

    $signature = 'RUQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
    $global = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=='
    $sigLines = @()
    $sigLines += 'untrusted comment: signature from orquestra release key'
    $sigLines += $signature
    $sigLines += "trusted comment: timestamp=1730000000`tfile=SHA256SUMS.txt`tversion=0.1.0-alpha.7"
    $sigLines += $global
    Write-Lines -Path $sig -Lines $sigLines
    Assert-TrustedComment -SignaturePath $sig -ExpectedVersion '0.1.0-alpha.7'

    $wrongVersion = $false
    try { Assert-TrustedComment -SignaturePath $sig -ExpectedVersion '0.1.0-alpha.6' }
    catch { $wrongVersion = $true }
    if (-not $wrongVersion) { throw 'trusted comment accepted a replayed version' }

    # A second `version=` field must not let an older tag ride along.
    $duplicateLines = @()
    $duplicateLines += 'untrusted comment: signature from orquestra release key'
    $duplicateLines += $signature
    $duplicateLines += "trusted comment: timestamp=1730000000`tfile=SHA256SUMS.txt`tversion=0.1.0-alpha.7`tversion=0.1.0-alpha.8"
    $duplicateLines += $global
    Write-Lines -Path $sig -Lines $duplicateLines
    $duplicateField = $false
    try { Assert-TrustedComment -SignaturePath $sig -ExpectedVersion '0.1.0-alpha.7' }
    catch { $duplicateField = $true }
    if (-not $duplicateField) { throw 'trusted comment accepted a duplicate field' }

    Write-Host 'PowerShell installer parsing tests passed.' -ForegroundColor Green
}
finally {
    Remove-Item -Path $root -Recurse -Force -ErrorAction SilentlyContinue
}
