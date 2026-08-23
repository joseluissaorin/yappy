# Sign Yappy's Windows installer + EXE with a code-signing certificate.
#
# Usage (local dev with PFX on disk):
#   $env:WINDOWS_PFX_PATH      = "C:\path\to\cert.pfx"
#   $env:WINDOWS_PFX_PASSWORD  = "..."
#   pwsh ./scripts/sign-windows.ps1 -BundleDir "yappy-app/src-tauri/target/release/bundle"
#
# Usage (CI, certificate base64 in secrets):
#   $env:WINDOWS_PFX_BASE64    = "<base64-encoded .pfx blob>"
#   $env:WINDOWS_PFX_PASSWORD  = "<password>"
#   pwsh ./scripts/sign-windows.ps1 -BundleDir $env:GITHUB_WORKSPACE\_release_uploads
#
# Optional:
#   $env:WINDOWS_SIGN_SUBJECT  = "CN=Yappy"   # auto-pick from cert store instead
#   $env:WINDOWS_TIMESTAMP_URL = "http://timestamp.digicert.com"
#
# What it signs: every .exe and .msi under -BundleDir, recursively. The MSI
# wraps already-signed EXEs, so we sign EXEs first then the MSI, which is the
# order signtool's documentation recommends and what works for Mark of the
# Web propagation on Windows 11 SmartScreen.
#
# Without a cert, Yappy still runs but SmartScreen shows "Unknown publisher"
# on first launch — users have to click "More info → Run anyway". Signed
# builds get the publisher name straight away, and after enough downloads
# Microsoft's reputation system auto-suppresses the warning entirely.

[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [string]$BundleDir,
    [string]$Subject,
    [string]$TimestampUrl = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"

# Find signtool.exe — ships with Windows 10 SDK, lives under "Program Files (x86)".
$signtool = $null
$sdkRoot = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
if (Test-Path $sdkRoot) {
    $signtool = Get-ChildItem -Path $sdkRoot -Recurse -Filter "signtool.exe" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "x64\\signtool.exe$" } |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $signtool) {
    throw "signtool.exe not found. Install Windows 10/11 SDK (https://aka.ms/windowssdk)."
}
Write-Host "Using signtool: $signtool"

# Resolve the certificate. Priority:
#   1) WINDOWS_PFX_BASE64 (CI mode) — decode to a temp .pfx
#   2) WINDOWS_PFX_PATH (local mode) — use as-is
#   3) WINDOWS_SIGN_SUBJECT or -Subject param — search the LocalMachine\My store
$pfxPath = $null
$pfxPassword = $env:WINDOWS_PFX_PASSWORD
$tempPfx = $null

if ($env:WINDOWS_PFX_BASE64) {
    $tempPfx = Join-Path ([System.IO.Path]::GetTempPath()) "yappy-sign-$(Get-Random).pfx"
    [System.IO.File]::WriteAllBytes($tempPfx, [System.Convert]::FromBase64String($env:WINDOWS_PFX_BASE64))
    $pfxPath = $tempPfx
    Write-Host "Loaded PFX from WINDOWS_PFX_BASE64 → $pfxPath"
} elseif ($env:WINDOWS_PFX_PATH) {
    $pfxPath = $env:WINDOWS_PFX_PATH
    Write-Host "Using PFX at $pfxPath"
}

$signArgs = @(
    "sign",
    "/fd", "SHA256",
    "/td", "SHA256",
    "/tr", $TimestampUrl,
    "/v"
)
if ($pfxPath) {
    if (-not $pfxPassword) { throw "PFX provided but WINDOWS_PFX_PASSWORD is empty." }
    $signArgs += "/f", $pfxPath, "/p", $pfxPassword
} else {
    $effectiveSubject = if ($Subject) { $Subject } else { $env:WINDOWS_SIGN_SUBJECT }
    if (-not $effectiveSubject) {
        throw "No certificate source. Set WINDOWS_PFX_BASE64, WINDOWS_PFX_PATH, or WINDOWS_SIGN_SUBJECT."
    }
    $signArgs += "/n", $effectiveSubject
    Write-Host "Signing with cert store subject: $effectiveSubject"
}

try {
    if (-not (Test-Path $BundleDir)) {
        throw "BundleDir not found: $BundleDir"
    }

    # EXEs first (NSIS installer + portable), then MSIs. This order propagates
    # signatures into the MSI's wrapped payload correctly.
    $targets = @()
    $targets += Get-ChildItem -Path $BundleDir -Recurse -Include *.exe -File -ErrorAction SilentlyContinue
    $targets += Get-ChildItem -Path $BundleDir -Recurse -Include *.msi -File -ErrorAction SilentlyContinue
    if (-not $targets) {
        Write-Warning "No .exe or .msi found under $BundleDir — nothing to sign."
        return
    }

    foreach ($f in $targets) {
        Write-Host ""
        Write-Host "── Signing: $($f.FullName)"
        & $signtool @signArgs $f.FullName
        if ($LASTEXITCODE -ne 0) {
            throw "signtool failed on $($f.FullName) (exit $LASTEXITCODE)"
        }
        # Verify the signature roundtripped.
        & $signtool verify /pa /v $f.FullName | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "Signature verify failed on $($f.FullName)"
        }
    }

    Write-Host ""
    Write-Host "Signed $($targets.Count) artifact(s)."
} finally {
    if ($tempPfx -and (Test-Path $tempPfx)) {
        Remove-Item -Force $tempPfx
    }
}
