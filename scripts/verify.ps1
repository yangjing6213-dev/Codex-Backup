param(
    [switch]$SkipRust,
    [switch]$SkipLocalRestic
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Results = [System.Collections.Generic.List[object]]::new()

$localCargoBin = Join-Path $ProjectRoot ".toolchain\cargo\bin"
if (Test-Path -LiteralPath (Join-Path $localCargoBin "cargo.exe")) {
    $env:RUSTUP_HOME = Join-Path $ProjectRoot ".toolchain\rustup"
    $env:CARGO_HOME = Join-Path $ProjectRoot ".toolchain\cargo"
    $env:Path = $localCargoBin + ";" + $env:Path
}

function Invoke-Verification([string]$Name, [scriptblock]$Action) {
    Write-Host "[verify] $Name"
    try {
        & $Action
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE" }
        $Results.Add([pscustomobject]@{ Name = $Name; Status = "PASS"; Detail = "completed" })
    } catch {
        Write-Warning "${Name}: $($_.Exception.Message)"
        $Results.Add([pscustomobject]@{ Name = $Name; Status = "FAIL"; Detail = $_.Exception.Message })
    }
}

Push-Location (Join-Path $ProjectRoot "desktop")
try {
    Invoke-Verification "frontend typecheck" { & pnpm exec tsc -b }
    Invoke-Verification "frontend tests" { & pnpm exec vitest --run }
    Invoke-Verification "frontend production build" { & pnpm run build }
} finally {
    Pop-Location
}

Invoke-Verification "bilingual documentation contract" {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tests\readme_language_test.ps1")
}

Invoke-Verification "version consistency contract" {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tests\version_consistency_test.ps1")
}

Invoke-Verification "installer desktop icon contract" {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tests\installer_icon_test.ps1")
}

if (-not $SkipRust) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        Push-Location (Join-Path $ProjectRoot "desktop\src-tauri")
        try {
            Invoke-Verification "Rust formatting" { & cargo fmt --all -- --check }
            Invoke-Verification "Rust tests" { & cargo test --locked --offline }
        } finally {
            Pop-Location
        }
    } else {
        $Results.Add([pscustomobject]@{ Name = "Rust toolchain"; Status = "NOT_RUN"; Detail = "cargo is unavailable" })
    }
}

$forbidden = & rg -n "tauri-plugin-updater|@tauri-apps/plugin-updater|UpdateControl" (Join-Path $ProjectRoot "desktop") 2>$null
if ($LASTEXITCODE -eq 0 -and $forbidden) {
    $Results.Add([pscustomobject]@{ Name = "updater/telemetry source scan"; Status = "FAIL"; Detail = ($forbidden -join " | ") })
} else {
    $Results.Add([pscustomobject]@{ Name = "updater/telemetry source scan"; Status = "PASS"; Detail = "no updater integration found" })
}

if (-not $SkipLocalRestic) {
    Invoke-Verification "isolated restic local acceptance" {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ProjectRoot "tests\run-local-acceptance.ps1")
    }
}

$reportDirectory = Join-Path $ProjectRoot "docs\verification"
New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null
$report = @(
    "# Verification run",
    "",
    "Generated: $([DateTimeOffset]::Now.ToString('o'))",
    "",
    "| Check | Status | Detail |",
    "| --- | --- | --- |"
)
foreach ($result in $Results) {
    $detail = ([string]$result.Detail).Replace('|', '\|').Replace("`r", ' ').Replace("`n", ' ')
    $report += "| $($result.Name) | $($result.Status) | $detail |"
}
[System.IO.File]::WriteAllText((Join-Path $reportDirectory "verify-latest.md"), ($report -join "`r`n"), [System.Text.UTF8Encoding]::new($false))

$Results | Format-Table -AutoSize
if ($Results | Where-Object Status -eq "FAIL") { exit 1 }
