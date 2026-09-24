param(
    [switch]$SkipBootstrap
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$localCargoBin = Join-Path $ProjectRoot ".toolchain\cargo\bin"
if (Test-Path -LiteralPath (Join-Path $localCargoBin "cargo.exe")) {
    $env:RUSTUP_HOME = Join-Path $ProjectRoot ".toolchain\rustup"
    $env:CARGO_HOME = Join-Path $ProjectRoot ".toolchain\cargo"
    $env:Path = $localCargoBin + ";" + $env:Path
}

if (-not $SkipBootstrap) {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "scripts\bootstrap.ps1")
    if ($LASTEXITCODE -ne 0) { throw "bootstrap failed" }
}

if (-not [Environment]::Is64BitOperatingSystem) {
    throw "Windows x64 is required for the installer target"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo is required to generate the Windows installer"
}
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    throw "MSVC cl.exe is unavailable; open a Visual Studio x64 build environment before packaging"
}

$resourceRoot = Join-Path $ProjectRoot "desktop\src-tauri\resources"
foreach ($tool in @("restic.exe", "rclone.exe")) {
    if (-not (Test-Path -LiteralPath (Join-Path $resourceRoot $tool))) {
        throw "missing bundled runtime: $tool"
    }
}

& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tests\bundled_licenses_test.ps1")
if ($LASTEXITCODE -ne 0) { throw "bundled license verification failed" }

Push-Location (Join-Path $ProjectRoot "desktop")
try {
    & pnpm run build
    if ($LASTEXITCODE -ne 0) { throw "frontend build failed" }
    & pnpm tauri build --ci --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "Tauri Windows x64 build failed" }
} finally {
    Pop-Location
}

$nsisScript = Join-Path $ProjectRoot "desktop\src-tauri\target\x86_64-pc-windows-msvc\release\nsis\x64\installer.nsi"
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tests\bundled_licenses_test.ps1") -NsisScript $nsisScript
if ($LASTEXITCODE -ne 0) { throw "installer license inclusion verification failed" }

$bundleRoot = Join-Path $ProjectRoot "desktop\src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis"
$installer = Get-ChildItem -LiteralPath $bundleRoot -Filter "*.exe" -File | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $installer) { throw "Tauri did not produce an NSIS installer" }
$sha256 = [System.Security.Cryptography.SHA256]::Create()
$stream = [System.IO.File]::OpenRead($installer.FullName)
try {
    $hash = ([System.BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
} finally {
    $stream.Dispose()
    $sha256.Dispose()
}
$checksumPath = "$($installer.FullName).sha256"
[System.IO.File]::WriteAllText($checksumPath, "$hash  $($installer.Name)`r`n", [System.Text.UTF8Encoding]::new($false))
Write-Host "[package] installer: $($installer.FullName)"
Write-Host "[package] sha256: $hash"
Write-Host "[package] unsigned current-user NSIS package; no system install was performed"
