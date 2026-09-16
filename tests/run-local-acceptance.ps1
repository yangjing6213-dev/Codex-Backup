param(
    [string]$ReportPath = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Restic = Join-Path $ProjectRoot "desktop\src-tauri\resources\restic.exe"
if (-not (Test-Path -LiteralPath $Restic)) { throw "bundled restic.exe is missing" }
if ([string]::IsNullOrWhiteSpace($ReportPath)) { $ReportPath = Join-Path $ProjectRoot "docs\verification\local-restic-acceptance.md" }

$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("enhe-local-acceptance-" + [guid]::NewGuid().ToString("N"))
$sourceRoot = Join-Path $testRoot "source"
$projectRoot = Join-Path $sourceRoot "demo project"
$worktreeRoot = Join-Path $sourceRoot "demo worktree"
$codexRoot = Join-Path $sourceRoot "codex data"
$payloadRoot = Join-Path $testRoot "enhe-payload"
$repository = Join-Path $testRoot "repository"
$restoreRoot = Join-Path $testRoot "restore"
$passwordFile = Join-Path $testRoot "password"
$reportDirectory = Split-Path -Parent $ReportPath
$evidence = [System.Collections.Generic.List[string]]::new()

function Invoke-Git([string[]]$Arguments) {
    $output = & git @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw "git $($Arguments -join ' ') failed: $($output -join ' ')" }
    return $output
}

function Invoke-Restic([string[]]$Arguments, [string]$WorkingDirectory = "") {
    if ($WorkingDirectory) { Push-Location $WorkingDirectory }
    try {
        $output = & $Restic @Arguments 2>&1
        if ($LASTEXITCODE -ne 0) { throw "restic $($Arguments -join ' ') failed: $($output -join ' ')" }
        return ($output -join "`n")
    } finally {
        if ($WorkingDirectory) { Pop-Location }
    }
}

function Copy-TreeContents([string]$Source, [string]$Destination) {
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Get-ChildItem -LiteralPath $Source -Force | Copy-Item -Destination $Destination -Recurse -Force
}

try {
    New-Item -ItemType Directory -Force -Path $projectRoot, $worktreeRoot, $codexRoot, $payloadRoot, $repository, $restoreRoot | Out-Null
    Set-Content -LiteralPath $passwordFile -Value "synthetic-local-password" -NoNewline
    Set-Content -LiteralPath (Join-Path $projectRoot "README.md") -Value "tracked project content"
    Invoke-Git @("-C", $projectRoot, "init", "--quiet") | Out-Null
    Invoke-Git @("-C", $projectRoot, "config", "user.email", "enhe-test@example.invalid") | Out-Null
    Invoke-Git @("-C", $projectRoot, "config", "user.name", "ENHE synthetic test") | Out-Null
    Invoke-Git @("-C", $projectRoot, "add", ".") | Out-Null
    Invoke-Git @("-C", $projectRoot, "commit", "--quiet", "-m", "initial synthetic commit") | Out-Null
    Set-Content -LiteralPath (Join-Path $projectRoot "uncommitted.txt") -Value "uncommitted change"
    Set-Content -LiteralPath (Join-Path $projectRoot "untracked.txt") -Value "untracked change"
    Invoke-Git @("-C", $projectRoot, "worktree", "add", "--quiet", $worktreeRoot, "-b", "enhe-synthetic-worktree") | Out-Null
    Set-Content -LiteralPath (Join-Path $worktreeRoot "worktree-change.txt") -Value "worktree content"

    New-Item -ItemType Directory -Force -Path (Join-Path $codexRoot "sessions") | Out-Null
    $partialJsonl = '{"id":"active"}' + [Environment]::NewLine + '{"id":"partial"'
    Set-Content -LiteralPath (Join-Path $codexRoot "sessions\active.jsonl") -Value $partialJsonl -NoNewline
    Set-Content -LiteralPath (Join-Path $codexRoot "archived.jsonl") -Value '{"id":"archived"}'
    Set-Content -LiteralPath (Join-Path $codexRoot "state.sqlite") -Value "synthetic sqlite main"
    Set-Content -LiteralPath (Join-Path $codexRoot "state.sqlite-wal") -Value "synthetic sqlite wal"
    Set-Content -LiteralPath (Join-Path $codexRoot "state.sqlite-shm") -Value "synthetic sqlite shm"
    Set-Content -LiteralPath (Join-Path $codexRoot "handoff.md") -Value "handoff and development notes"

    $projectsPayload = Join-Path $payloadRoot "projects"
    Copy-TreeContents $projectRoot (Join-Path $projectsPayload "demo-project")
    Copy-TreeContents $worktreeRoot (Join-Path $projectsPayload "demo-worktree")
    Copy-TreeContents $codexRoot (Join-Path $payloadRoot "codex")
    New-Item -ItemType Directory -Force -Path (Join-Path $payloadRoot "git-metadata") | Out-Null
    Set-Content -LiteralPath (Join-Path $payloadRoot "manifest.json") -Value '{"format":"enhe-codex-backup","schema_version":1,"integrity_status":"complete"}'

    $initOutput = Invoke-Restic @("--repo", $repository, "--password-file", $passwordFile, "init")
    $backupOutput = Invoke-Restic @("--repo", $repository, "--password-file", $passwordFile, "backup", "--json", "--tag", "enhe-codex-backup", "enhe-payload") $testRoot
    $snapshot = $null
    foreach ($line in ($backupOutput -split "`r?`n")) {
        if ($line.TrimStart().StartsWith("{")) {
            try {
                $candidate = $line | ConvertFrom-Json
                if ($candidate.snapshot_id) { $snapshot = [string]$candidate.snapshot_id }
            } catch { }
        }
    }
    if (-not $snapshot) { throw "restic did not return a snapshot id" }
    Invoke-Restic @("--repo", $repository, "--password-file", $passwordFile, "check") | Out-Null
    $snapshotList = Invoke-Restic @("--repo", $repository, "--password-file", $passwordFile, "snapshots", "--json") | ConvertFrom-Json
    if (@($snapshotList).Count -ne 1) { throw "expected one local snapshot" }
    Invoke-Restic @("--repo", $repository, "--password-file", $passwordFile, "restore", $snapshot, "--target", $restoreRoot) | Out-Null

    $checks = [ordered]@{
        "encrypted repository initialized" = (Test-Path -LiteralPath (Join-Path $repository "config"))
        "snapshot was created" = ($snapshot.Length -ge 8)
        "Git metadata restored" = (Test-Path -LiteralPath (Join-Path $restoreRoot "enhe-payload\projects\demo-project\.git\config"))
        "uncommitted file restored" = (Test-Path -LiteralPath (Join-Path $restoreRoot "enhe-payload\projects\demo-project\uncommitted.txt"))
        "worktree file restored" = (Test-Path -LiteralPath (Join-Path $restoreRoot "enhe-payload\projects\demo-worktree\worktree-change.txt"))
        "active JSONL restored" = (Test-Path -LiteralPath (Join-Path $restoreRoot "enhe-payload\codex\sessions\active.jsonl"))
        "SQLite WAL sidecar restored" = (Test-Path -LiteralPath (Join-Path $restoreRoot "enhe-payload\codex\state.sqlite-wal"))
        "handoff notes restored" = (Test-Path -LiteralPath (Join-Path $restoreRoot "enhe-payload\codex\handoff.md"))
    }
    foreach ($check in $checks.GetEnumerator()) {
        $status = if ($check.Value) { "PASS" } else { "FAIL" }
        $evidence.Add("| $($check.Key) | $status |")
        if (-not $check.Value) { throw "$($check.Key) failed" }
    }
    $evidence.Insert(0, "| Check | Status |`r`n| --- | --- |`r`n")
    $evidence.Add("")
    $evidence.Add("This is a real restic CLI test against a temporary encrypted repository. It validates the storage engine and restore layout only; it does not claim Rust/Tauri application integration, OneDrive E2E, account migration, or second-device continuation.")
    New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null
    $reportLines = @("# Isolated local restic acceptance", "", "Generated: $([DateTimeOffset]::Now.ToString('o'))", "", "Synthetic fixture: Git repository with uncommitted and untracked files, linked worktree, active/partial JSONL, SQLite WAL/SHM sidecars, archived JSONL, and handoff notes.", "", ($evidence -join "`r`n"))
    [System.IO.File]::WriteAllText($ReportPath, ($reportLines -join "`r`n"), [System.Text.UTF8Encoding]::new($false))
    Write-Output "PASS isolated restic local acceptance; snapshot=$snapshot"
} finally {
    if (Test-Path -LiteralPath $testRoot) { Remove-Item -LiteralPath $testRoot -Recurse -Force }
}
