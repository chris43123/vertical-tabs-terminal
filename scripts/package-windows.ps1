# Build the Windows release packages into dist\:
#   vtt-<version>-windows-x86_64.zip         portable: unzip and run vtt.exe
#   vtt-<version>-windows-x86_64-setup.exe   installer (needs Inno Setup 6: winget install JRSoftware.InnoSetup)
#
# Usage: pwsh scripts/package-windows.ps1
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$dist = 'dist'
New-Item -ItemType Directory -Force -Path $dist | Out-Null

cargo build --release --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Portable zip.
$name = "vtt-$version-windows-x86_64"
$stage = Join-Path ([IO.Path]::GetTempPath()) $name
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item target\release\vtt.exe, README.md, LICENSE, config.example.toml $stage
$zip = Join-Path $dist "$name.zip"
Remove-Item -Force $zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Write-Host "Built $zip"

# Installer, when Inno Setup is available.
$iscc = (Get-Command iscc -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if ($iscc) {
    & $iscc /Q "/DAppVersion=$version" packaging\windows\vtt.iss
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "Built $dist\$name-setup.exe"
} else {
    Write-Host 'Skipping the installer: install Inno Setup 6 (winget install JRSoftware.InnoSetup)'
}
