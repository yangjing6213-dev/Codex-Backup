param(
    [switch]$RunNsisFixture,
    [string]$MakeNsisPath
)

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
    "target-inherited icon" = 'CreateShortcut\s+"\$DESKTOP\\\$\{PRODUCTNAME\}\.lnk"\s+"\$INSTDIR\\\$\{MAINBINARYNAME\}\.exe"\s+""\s+""\s+""'
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

Write-Host "Installer desktop icon configuration contract: PASS"
if (-not $RunNsisFixture) {
    Write-Host "Native shortcut fixture: NOT_RUN (use -RunNsisFixture with the Tauri NSIS toolchain)"
    return
}
if (-not $MakeNsisPath) {
    $MakeNsisPath = Join-Path $env:LOCALAPPDATA "tauri\NSIS\makensis.exe"
}
if (-not (Test-Path -LiteralPath $MakeNsisPath -PathType Leaf)) {
    throw "NSIS is required for the shortcut behavior test. Pass -MakeNsisPath or prepare the Tauri bundler."
}

# Execute the real hook, redirecting only its Desktop destination into a fixture.
# Application identity is unrelated to icon resolution and stays a source contract above.
$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ("enhe-shortcut-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $fixtureRoot | Out-Null
$fixtureHook = $hook.Replace('$DESKTOP', '$EXEDIR\desktop')
$nsi = @'
Unicode true
RequestExecutionLevel user
SilentInstall silent
Name "ENHE shortcut regression fixture"
OutFile "shortcut-fixture.exe"
!include "LogicLib.nsh"
!define PRODUCTNAME "ENHE shortcut fixture"
!define MAINBINARYNAME "notepad"
!macro SetLnkAppUserModelId Shortcut
!macroend
!include "hook-under-test.nsh"
Section
  StrCpy $INSTDIR "$WINDIR\System32"
  SetOutPath "$INSTDIR"
  !insertmacro NSIS_HOOK_POSTINSTALL
SectionEnd
'@
[IO.File]::WriteAllText((Join-Path $fixtureRoot "hook-under-test.nsh"), $fixtureHook)
[IO.File]::WriteAllText((Join-Path $fixtureRoot "fixture.nsi"), $nsi)
& $MakeNsisPath /V2 (Join-Path $fixtureRoot "fixture.nsi")
if ($LASTEXITCODE -ne 0) { throw "Could not compile the real NSIS hook fixture." }

$fixtureExe = Join-Path $fixtureRoot "shortcut-fixture.exe"
$fixtureDesktop = Join-Path $fixtureRoot "desktop"
New-Item -ItemType Directory -Path $fixtureDesktop | Out-Null
$shortcutPath = Join-Path $fixtureDesktop "ENHE shortcut fixture.lnk"
$process = Start-Process -FilePath $fixtureExe -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "NSIS fixture failed." }
if (Test-Path -LiteralPath $shortcutPath) {
    throw "The hook must preserve the choice not to create a desktop shortcut."
}

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = Join-Path $env:WINDIR "System32\notepad.exe"
$shortcut.IconLocation = (Join-Path $fixtureRoot "missing-old-icon.ico") + ",0"
$shortcut.Save()
$process = Start-Process -FilePath $fixtureExe -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "NSIS fixture failed while refreshing the shortcut." }
$refreshed = $shell.CreateShortcut($shortcutPath)
if (-not (Test-Path -LiteralPath $refreshed.TargetPath -PathType Leaf)) {
    throw "The refreshed shortcut has no executable target."
}
if ($refreshed.IconLocation -notin @("", ",0")) {
    throw "The shortcut pins a separate icon path ($($refreshed.IconLocation)); redirected installs must inherit the resolved target icon."
}
Write-Host "Installer desktop icon contract: PASS (real NSIS shortcut refresh and no-shortcut preference)"
Write-Host "Synthetic fixture retained at: $fixtureRoot"
