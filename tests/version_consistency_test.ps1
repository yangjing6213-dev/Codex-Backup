$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$expectedVersion = "0.1.6"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

$package = Get-Content -LiteralPath (Join-Path $projectRoot "desktop\package.json") -Raw | ConvertFrom-Json
if ([string]$package.version -ne $expectedVersion) {
    throw "desktop/package.json version is $($package.version), expected $expectedVersion."
}

$tauri = Get-Content -LiteralPath (Join-Path $projectRoot "desktop\src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
if ([string]$tauri.version -ne $expectedVersion) {
    throw "tauri.conf.json version is $($tauri.version), expected $expectedVersion."
}
if ([string]$tauri.app.windows[0].title -ne "ENHE Codex Backup · v$expectedVersion") {
    throw "The desktop window title does not contain v$expectedVersion."
}

$cargoManifest = Get-Content -LiteralPath (Join-Path $projectRoot "desktop\src-tauri\Cargo.toml") -Raw
if ($cargoManifest -notmatch "(?ms)^\[package\]\s+name\s*=\s*`"enhe-codex-backup`"\s+version\s*=\s*`"$([regex]::Escape($expectedVersion))`"") {
    throw "Cargo.toml package version is not $expectedVersion."
}

$cargoLock = Get-Content -LiteralPath (Join-Path $projectRoot "desktop\src-tauri\Cargo.lock") -Raw
if ($cargoLock -notmatch "(?ms)\[\[package\]\]\s+name\s*=\s*`"enhe-codex-backup`"\s+version\s*=\s*`"$([regex]::Escape($expectedVersion))`"") {
    throw "Cargo.lock application package version is not $expectedVersion."
}

$appSource = Get-Content -LiteralPath (Join-Path $projectRoot "desktop\src\App.tsx") -Raw
foreach ($requiredText in @("brand-version`">v$expectedVersion", "当前版本`")} $expectedVersion")) {
    if (-not $appSource.Contains($requiredText)) {
        throw "App.tsx is missing the visible version marker: $requiredText"
    }
}

$changelog = Get-Content -LiteralPath (Join-Path $projectRoot "CHANGELOG.md") -Raw
if (-not $changelog.Contains("## [$expectedVersion] - 2026-09-20")) {
    throw "CHANGELOG.md is missing the $expectedVersion release section."
}

$readme = Get-Content -LiteralPath (Join-Path $projectRoot "README.md") -Raw
$expectedChineseVersion = '当前版本：`' + $expectedVersion + '`'
if (-not $readme.Contains($expectedChineseVersion)) {
    throw "README.md does not identify $expectedVersion as the current version."
}

$englishReadme = Get-Content -LiteralPath (Join-Path $projectRoot "README.en.md") -Raw
$expectedEnglishVersion = 'Current version: `' + $expectedVersion + '`'
if (-not $englishReadme.Contains($expectedEnglishVersion)) {
    throw "README.en.md does not identify $expectedVersion as the current version."
}

Write-Host "Version consistency contract: PASS ($expectedVersion)"
