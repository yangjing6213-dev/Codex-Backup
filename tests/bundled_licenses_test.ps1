param(
    [string]$ProjectRoot = (Join-Path $PSScriptRoot ".."),
    [string]$NsisScript
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$ProjectRoot = (Resolve-Path -LiteralPath $ProjectRoot).Path
$tauriRoot = Join-Path $ProjectRoot "desktop/src-tauri"
$noticeRoot = Join-Path $tauriRoot "resources/licenses"
$manifestPath = Join-Path $noticeRoot "manifest.json"
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Bundled license manifest is missing. Do not distribute this installer."
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
$config = Get-Content -LiteralPath (Join-Path $tauriRoot "tauri.conf.json") -Raw -Encoding UTF8 | ConvertFrom-Json
function Get-ContentHash([string]$Path, [switch]$NormalizeText) {
    # Lockfiles can be checked out with CRLF or LF. License payloads below use raw hashes.
    $sha = [Security.Cryptography.SHA256]::Create()
    $stream = $null
    try {
        if ($NormalizeText) {
            $bytes = [Text.Encoding]::UTF8.GetBytes([IO.File]::ReadAllText($Path).Replace("`r`n", "`n"))
            $hash = $sha.ComputeHash($bytes)
        } else {
            $stream = [IO.File]::OpenRead($Path)
            $hash = $sha.ComputeHash($stream)
        }
        return ([BitConverter]::ToString($hash)).Replace("-", "").ToLowerInvariant()
    } finally {
        if ($stream) { $stream.Dispose() }
        $sha.Dispose()
    }
}
$required = @("LICENSE.txt", "THIRD-PARTY-NOTICES.txt", "JS-THIRD-PARTY-NOTICES.txt", "RUST-THIRD-PARTY-NOTICES.txt", "GO-THIRD-PARTY-NOTICES.txt")
$sourceMaterials = @("RCLONE-LGPL-SOURCE-AND-RELINKING.txt", "RCLONE-BUILD-MANIFEST.json", "RCLONE-LGPL-SOURCE-BUNDLE.zip")
foreach ($name in $required) {
    $entry = @($manifest.notices | Where-Object path -eq $name)
    if ($entry.Count -ne 1) { throw "Missing or duplicate notice entry: $name" }
    $path = Join-Path $noticeRoot $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing bundled notice: $name" }
    if ((Get-ContentHash $path) -ne $entry[0].sha256) {
        throw "Bundled notice changed without review: $name"
    }
}
foreach ($entry in @($manifest.sourceMaterials)) {
    if ($entry.path -notin $sourceMaterials) { throw "Unexpected source material entry: $($entry.path)" }
    $path = Join-Path $noticeRoot $entry.path
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing LGPL source material: $($entry.path)" }
    if ((Get-ContentHash $path) -ne $entry.sha256) {
        throw "LGPL source material changed without review: $($entry.path)"
    }
    if ((Get-Item -LiteralPath $path).Length -ne [int64]$entry.bytes) {
        throw "LGPL source material size changed without review: $($entry.path)"
    }
}
if (@($manifest.sourceMaterials).Count -ne $sourceMaterials.Count -or
    @($manifest.sourceMaterials.path | Sort-Object -Unique).Count -ne $sourceMaterials.Count) {
    throw "LGPL source materials must include one source bundle, relinking instructions and build manifest."
}
$buildManifestPath = Join-Path $noticeRoot "RCLONE-BUILD-MANIFEST.json"
$buildManifest = Get-Content -LiteralPath $buildManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($buildManifest.component -ne "github.com/rclone/rclone" -or
    $buildManifest.rclone.sourceCommit -ne "687d264b689b8c49a67e2e52a8a5e0caa01c04ce" -or
    $buildManifest.sddl.sourceCommit -ne "926454e91efc95daf839e9112b09b6b2344c9e83" -or
    [string]::IsNullOrWhiteSpace($buildManifest.relinking.command)) {
    throw "LGPL build manifest does not identify the exact rclone/SDDL source and relinking command."
}
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::OpenRead((Join-Path $noticeRoot "RCLONE-LGPL-SOURCE-BUNDLE.zip"))
try {
    $archiveNames = @($archive.Entries | ForEach-Object FullName)
    foreach ($requiredEntry in @("rclone-source/go.mod", "sddl-source/LICENSE")) {
        if ($requiredEntry -notin $archiveNames) { throw "LGPL source bundle is missing: $requiredEntry" }
    }
} finally {
    $archive.Dispose()
}
foreach ($name in ($required + $sourceMaterials + "manifest.json")) {
    if (@($config.bundle.resources) -notcontains "resources/licenses/$name") {
        throw "License/source material is not included as a Tauri bundle resource: $name"
    }
}
if ((Get-ContentHash (Join-Path $ProjectRoot "LICENSE") -NormalizeText) -ne
    (Get-ContentHash (Join-Path $noticeRoot "LICENSE.txt") -NormalizeText)) {
    throw "Bundled program license must preserve the root LICENSE text."
}
foreach ($entry in $manifest.inputs) {
    if ($entry.path -notin @("LICENSE", "desktop/pnpm-lock.yaml", "desktop/src-tauri/Cargo.lock", "desktop/src-tauri/resources/restic.exe", "desktop/src-tauri/resources/rclone.exe")) {
        throw "Unexpected license input path."
    }
    $path = Join-Path $ProjectRoot $entry.path
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "License inventory input is missing: $($entry.path)"
    }
    $hash = Get-ContentHash $path -NormalizeText:(-not $entry.path.EndsWith(".exe"))
    if ($hash -ne $entry.sha256) {
        throw "License inventory input changed; refresh notices before packaging: $($entry.path)"
    }
}
if (@($manifest.inputs).Count -ne 5 -or @($manifest.inputs.path | Sort-Object -Unique).Count -ne 5) {
    throw "License inventory must cover the program, both lockfiles and both runtime binaries."
}
if ($NsisScript) {
    $script = Get-Content -LiteralPath $NsisScript -Raw -Encoding UTF8
    foreach ($name in ($required + $sourceMaterials + "manifest.json")) {
        $source = (Join-Path $noticeRoot $name).Replace('/', '\')
        if ($script -notmatch ('(?m)^\s*File\s+[^\r\n]*"' + [regex]::Escape($source) + '"\s*$')) {
            throw "Generated installer does not embed notice: $name"
        }
    }
}
Write-Host "Bundled licenses: PASS (program, JS, Rust, Go; LGPL source/relink materials; exact hashes and Tauri resources)"
