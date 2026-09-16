$ErrorActionPreference = "Stop"

$Repo = Split-Path -Parent $PSScriptRoot
$ChinesePath = Join-Path $Repo "README.md"
$EnglishPath = Join-Path $Repo "README.en.md"
$Chinese = Get-Content -LiteralPath $ChinesePath -Raw -Encoding UTF8
$English = Get-Content -LiteralPath $EnglishPath -Raw -Encoding UTF8

if (-not $Chinese.Contains("[English](README.en.md)")) { throw "Chinese README does not link to README.en.md" }
if (-not $English.Contains("](README.md)")) { throw "English README does not link to README.md" }
foreach ($Phrase in @("# ENHE Codex Backup", "docs/USER_GUIDE.zh-CN.md", "docs/OFFLINE_RESTORE.zh-CN.md", "docs/STATUS.md")) {
    if (-not $Chinese.Contains($Phrase)) { throw "Chinese README is missing: $Phrase" }
}
foreach ($Phrase in @("Backups & Migration", "cloud off by default", "complete local backup", "docs/USER_GUIDE.en.md", "docs/OFFLINE_RESTORE.en.md")) {
    if (-not $English.Contains($Phrase)) { throw "English README is missing: $Phrase" }
}
foreach ($Path in @("docs/USER_GUIDE.zh-CN.md", "docs/USER_GUIDE.en.md", "docs/OFFLINE_RESTORE.zh-CN.md", "docs/OFFLINE_RESTORE.en.md", "docs/COMPATIBILITY.md", "docs/ACCEPTANCE.md", "docs/UPSTREAM.md")) {
    if (-not (Test-Path -LiteralPath (Join-Path $Repo $Path))) { throw "required document is missing: $Path" }
}
Write-Output "PASS bilingual README contract"
