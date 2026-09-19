$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$tauriRoot = Join-Path $projectRoot "desktop\src-tauri"
$configPath = Join-Path $tauriRoot "tauri.conf.json"
$config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json

if (-not ($config.bundle.windows.nsis.PSObject.Properties.Name -contains "installerHooks")) {
    throw "NSIS installerHooks is not configured."
}

$hookRelativePath = [string]$config.bundle.windows.nsis.installerHooks
if ($hookRelativePath -ne "installer-hooks.nsh") {
    throw "Unexpected NSIS installer hook path: $hookRelativePath"
}

$hookPath = Join-Path $tauriRoot $hookRelativePath
if (-not (Test-Path -LiteralPath $hookPath -PathType Leaf)) {
    throw "NSIS installer hook is missing: $hookPath"
}

$hook = Get-Content -LiteralPath $hookPath -Raw
$requiredPatterns = [ordered]@{
    "post-install hook" = '!macro\s+NSIS_HOOK_POSTINSTALL'
    "existing shortcut guard" = '\$\{FileExists\}\s+"\$DESKTOP\\\$\{PRODUCTNAME\}\.lnk"'
    "explicit ENHE executable icon" = 'CreateShortcut\s+"\$DESKTOP\\\$\{PRODUCTNAME\}\.lnk"\s+"\$INSTDIR\\\$\{MAINBINARYNAME\}\.exe"\s+""\s+"\$INSTDIR\\\$\{MAINBINARYNAME\}\.exe"\s+0'
    "application identity" = 'SetLnkAppUserModelId'
    "Windows icon cache notification" = 'SHChangeNotify\(i\s+0x08000000'
}

foreach ($entry in $requiredPatterns.GetEnumerator()) {
    if ($hook -notmatch $entry.Value) {
        throw "NSIS installer hook is missing $($entry.Key)."
    }
}

$guardPosition = $hook.IndexOf('${FileExists}', [StringComparison]::Ordinal)
$shortcutPosition = $hook.IndexOf('CreateShortcut', [StringComparison]::Ordinal)
if ($guardPosition -lt 0 -or $shortcutPosition -lt 0 -or $guardPosition -gt $shortcutPosition) {
    throw "The existing-shortcut guard must run before shortcut recreation."
}

if ($hook -match 'Delete\s+"\$DESKTOP\\\$\{PRODUCTNAME\}\.lnk"') {
    throw "The existing shortcut must not be deleted before its replacement is ready."
}

if (-not (@($config.bundle.icon) -contains "icons/icon.ico")) {
    throw "The Windows application icon is not configured."
}

$iconPath = Join-Path $tauriRoot "icons\icon.ico"
if (-not (Test-Path -LiteralPath $iconPath -PathType Leaf)) {
    throw "The generated ENHE Windows icon is missing."
}

Write-Host "Installer desktop icon contract: PASS"
