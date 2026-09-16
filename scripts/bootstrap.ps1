param(
    [switch]$Force,
    [switch]$SkipNodeInstall
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
Set-StrictMode -Version Latest

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DownloadRoot = Join-Path $ProjectRoot ".toolchain\downloads"
$ExtractRoot = Join-Path $ProjectRoot ".toolchain\extracted"
$ResourceRoot = Join-Path $ProjectRoot "desktop\src-tauri\resources"

$ResticVersion = "0.19.1"
$ResticArchive = "restic_${ResticVersion}_windows_amd64.zip"
$ResticUrl = "https://github.com/restic/restic/releases/download/v$ResticVersion/$ResticArchive"
$ResticSha256 = "da948ad707ed690426473aaba2046cd61f8f90f6f0e7dab6be0d5796531de67d"

$RcloneVersion = "1.75.1"
$RcloneArchive = "rclone-v${RcloneVersion}-windows-amd64.zip"
$RcloneUrl = "https://downloads.rclone.org/v$RcloneVersion/$RcloneArchive"
$RcloneSha256 = "200eb602c126d82aa38b51e0f6b9ae837473ff99b51278d3f6f837574c494d6e"

function Write-Stage([string]$Message) {
    Write-Host "[bootstrap] $Message"
}

function Get-VerifiedArchive([string]$Name, [string]$Url, [string]$ExpectedSha256) {
    $destination = Join-Path $DownloadRoot $Name
    if ($Force -or -not (Test-Path -LiteralPath $destination)) {
        Write-Stage "downloading $Name from the pinned official release"
        Invoke-WebRequest -Uri $Url -OutFile $destination -UseBasicParsing
    }
    $actual = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $ExpectedSha256) {
        throw "$Name SHA-256 mismatch: expected $ExpectedSha256, got $actual"
    }
    Write-Stage "$Name verified: $actual"
    return $destination
}

New-Item -ItemType Directory -Force -Path $DownloadRoot, $ExtractRoot, $ResourceRoot | Out-Null

if (-not $SkipNodeInstall) {
    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
        throw "pnpm is required for the frontend dependency check"
    }
    Push-Location (Join-Path $ProjectRoot "desktop")
    try {
        & pnpm install --frozen-lockfile --ignore-scripts
        if ($LASTEXITCODE -ne 0) { throw "pnpm dependency installation failed" }
    } finally {
        Pop-Location
    }
}

if (Get-Command git -ErrorAction SilentlyContinue) {
    Write-Stage (& git --version)
} else {
    throw "Git is required by the migration and synthetic acceptance checks"
}

if (Test-Path -LiteralPath (Join-Path $ProjectRoot ".toolchain\cargo\bin\cargo.exe")) {
    $env:RUSTUP_HOME = Join-Path $ProjectRoot ".toolchain\rustup"
    $env:CARGO_HOME = Join-Path $ProjectRoot ".toolchain\cargo"
    $env:Path = (Join-Path $ProjectRoot ".toolchain\cargo\bin") + ";" + $env:Path
    Write-Stage (& cargo --version)
    Write-Stage (& rustc --version)
} elseif (Get-Command cargo -ErrorAction SilentlyContinue) {
    Write-Stage (& cargo --version)
    Write-Stage (& rustc --version)
} else {
    Write-Stage "Rust is not available; source setup continues, Rust verification remains NOT_RUN"
}

$resticZip = Get-VerifiedArchive $ResticArchive $ResticUrl $ResticSha256
$rcloneZip = Get-VerifiedArchive $RcloneArchive $RcloneUrl $RcloneSha256

$resticExtract = Join-Path $ExtractRoot "restic-$ResticVersion"
$rcloneExtract = Join-Path $ExtractRoot "rclone-$RcloneVersion"
Expand-Archive -LiteralPath $resticZip -DestinationPath $resticExtract -Force
Expand-Archive -LiteralPath $rcloneZip -DestinationPath $rcloneExtract -Force

$resticBinary = Get-ChildItem -LiteralPath $resticExtract -Filter "*.exe" -File | Select-Object -First 1
$rcloneBinary = Get-ChildItem -LiteralPath $rcloneExtract -Filter "rclone.exe" -File -Recurse | Select-Object -First 1
if (-not $resticBinary -or -not $rcloneBinary) {
    throw "the pinned archives did not contain the expected Windows executables"
}
Copy-Item -LiteralPath $resticBinary.FullName -Destination (Join-Path $ResourceRoot "restic.exe") -Force
Copy-Item -LiteralPath $rcloneBinary.FullName -Destination (Join-Path $ResourceRoot "rclone.exe") -Force

Write-Stage (& (Join-Path $ResourceRoot "restic.exe") version)
Write-Stage (& (Join-Path $ResourceRoot "rclone.exe") version | Select-Object -First 1)
Write-Stage "verified runtime tools are in desktop/src-tauri/resources"
