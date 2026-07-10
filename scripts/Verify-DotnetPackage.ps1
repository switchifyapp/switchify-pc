$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'WinSigningTools.psm1') -Force -DisableNameChecking

$distDir = Resolve-ProjectPath 'dist'
$stageDir = Join-Path $distDir 'win-unpacked'
$appExe = Join-Path $stageDir 'Switchify PC.exe'
$launcherExe = Join-Path $stageDir 'Switchify PC Startup.exe'
$builderDebug = Join-Path $distDir 'builder-debug.yml'
$latestYml = Join-Path $distDir 'latest.yml'
$appProjectPath = Resolve-ProjectPath 'src' 'SwitchifyPc.App' 'SwitchifyPc.App.csproj'
$appVersion = Get-ProjectVersion -ProjectPath $appProjectPath
$expectedInstaller = Join-Path $distDir "Switchify-PC-Setup-$appVersion-x64.exe"
$installerScript = Resolve-ProjectPath 'installer' 'SwitchifyPc.DotNet.nsi'

function Assert-Exists {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label
  )

  if (-not (Test-Path -LiteralPath $Path)) {
    throw "$Label was not found: $Path"
  }
}

function Assert-NoLegacyRuntime {
  $markers = @(
    (Join-Path $stageDir 'resources/app.asar'),
    (Join-Path $stageDir 'chrome_100_percent.pak'),
    (Join-Path $stageDir 'ffmpeg.dll'),
    (Join-Path $stageDir ('resources/' + 'elect' + 'ron.asar'))
  )

  foreach ($marker in $markers) {
    if (Test-Path -LiteralPath $marker) {
      throw "C# package contains legacy runtime marker: $marker"
    }
  }
}

function Get-EmbeddedManifest {
  param([Parameter(Mandatory = $true)][string]$ExecutablePath)

  $mt = Get-WindowsSdkTool -ToolName 'mt.exe'
  $temporaryManifest = Join-Path ([System.IO.Path]::GetTempPath()) ("switchify-manifest-" + [Guid]::NewGuid().ToString('N') + '.xml')
  try {
    Invoke-ExternalTool -FilePath $mt -Arguments @('-nologo', "-inputresource:$ExecutablePath;#1", "-out:$temporaryManifest")
    return Get-Content -LiteralPath $temporaryManifest -Raw
  } finally {
    if (Test-Path -LiteralPath $temporaryManifest) {
      Remove-Item -LiteralPath $temporaryManifest -Force
    }
  }
}

function Assert-ExecutableManifest {
  param(
    [Parameter(Mandatory = $true)][string]$ExecutablePath,
    [Parameter(Mandatory = $true)][string]$ExpectedLevel,
    [Parameter(Mandatory = $true)][string]$ExpectedUiAccess
  )

  [xml]$manifest = Get-EmbeddedManifest -ExecutablePath $ExecutablePath
  $executionLevel = $manifest.SelectSingleNode("//*[local-name()='requestedExecutionLevel']")
  if (-not $executionLevel) {
    throw "Embedded manifest for $(Split-Path -Leaf $ExecutablePath) has no requestedExecutionLevel."
  }
  if ($executionLevel.level -ne $ExpectedLevel -or $executionLevel.uiAccess -ne $ExpectedUiAccess) {
    throw "Embedded manifest for $(Split-Path -Leaf $ExecutablePath) expected level=$ExpectedLevel uiAccess=$ExpectedUiAccess but found level=$($executionLevel.level) uiAccess=$($executionLevel.uiAccess)."
  }
}

function Assert-ExecutableSignature {
  param([Parameter(Mandatory = $true)][string]$ExecutablePath)

  $signature = Get-AuthenticodeSignature -LiteralPath $ExecutablePath
  if ($signature.Status -eq 'Valid') {
    return
  }

  if ($env:SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE -eq '1' -and $signature.Status -eq 'NotSigned') {
    Write-Warning "Skipping signature verification for unsigned development package: $ExecutablePath"
    return
  }

  throw "Authenticode signature is not valid for $ExecutablePath. Status: $($signature.Status)"
}

function Assert-InstallerScript {
  Assert-Exists -Path $installerScript -Label 'C# NSIS installer script'
  $content = Get-Content -LiteralPath $installerScript -Raw
  Assert-Includes -Content $content -Expected 'RequestExecutionLevel admin' -Label 'installer elevation requirement'
  Assert-Includes -Content $content -Expected 'InstallDir "$PROGRAMFILES64\Switchify PC"' -Label 'installer Program Files target'
  Assert-Includes -Content $content -Expected '!define MUI_FINISHPAGE_RUN "$INSTDIR\Switchify PC.exe"' -Label 'installer run-after-finish behavior'
  Assert-Includes -Content $content -Expected 'Call CloseRunningAppForInstall' -Label 'installer running-app close flow'
  Assert-Includes -Content $content -Expected 'Call un.CloseRunningAppForInstall' -Label 'uninstaller running-app close flow'
  Assert-Includes -Content $content -Expected 'find /I "${APP_EXE}"' -Label 'installer process detection'
  Assert-Includes -Content $content -Expected '--quit-for-install' -Label 'installer graceful app quit signal'
  Assert-Includes -Content $content -Expected 'taskkill /IM "${APP_EXE}" /F /T' -Label 'installer force-close fallback'
  Assert-Includes -Content $content -Expected 'IfSilent' -Label 'installer silent-mode handling'
  Assert-Includes -Content $content -Expected 'Exec ''"$INSTDIR\${APP_EXE}"''' -Label 'installer silent relaunch'
  Assert-Includes -Content $content -Expected 'silent_force_close:' -Label 'installer silent force-close path'
  Assert-Includes -Content $content -Expected 'SetErrorLevel 1' -Label 'installer silent failure exit code'
  Assert-Includes -Content $content -Expected 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC' -Label 'installer uninstall registry key'
  Assert-Includes -Content $content -Expected 'DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "app.switchify.pc"' -Label 'startup Run cleanup'
  Assert-Includes -Content $content -Expected 'DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "app.switchify.pc"' -Label 'startup approval cleanup'
  Assert-Includes -Content $content -Expected 'schtasks.exe /Delete /TN "Switchify PC" /F' -Label 'legacy startup task cleanup'
}

function Assert-Includes {
  param(
    [Parameter(Mandatory = $true)][string]$Content,
    [Parameter(Mandatory = $true)][string]$Expected,
    [Parameter(Mandatory = $true)][string]$Label
  )

  if (-not $Content.Contains($Expected)) {
    throw "$Label is missing $Expected."
  }
}

function Assert-Regex {
  param(
    [Parameter(Mandatory = $true)][string]$Content,
    [Parameter(Mandatory = $true)][string]$Regex,
    [Parameter(Mandatory = $true)][string]$Label
  )

  if ($Content -notmatch $Regex) {
    throw "$Label is missing."
  }
}

Assert-Exists -Path $appExe -Label 'staged C# app executable'
Assert-Exists -Path $launcherExe -Label 'staged startup launcher executable'
Assert-Exists -Path $builderDebug -Label 'C# builder debug metadata'
Assert-InstallerScript
Assert-NoLegacyRuntime
Assert-ExecutableManifest -ExecutablePath $appExe -ExpectedLevel 'highestAvailable' -ExpectedUiAccess 'true'
Assert-ExecutableManifest -ExecutablePath $launcherExe -ExpectedLevel 'asInvoker' -ExpectedUiAccess 'false'
Assert-ExecutableSignature -ExecutablePath $appExe
Assert-ExecutableSignature -ExecutablePath $launcherExe

if ((Test-Path -LiteralPath $expectedInstaller) -or (Test-Path -LiteralPath $latestYml)) {
  Assert-Exists -Path $expectedInstaller -Label 'C# NSIS installer'
  Assert-Exists -Path $latestYml -Label 'C# updater metadata'
  $latest = Get-Content -LiteralPath $latestYml -Raw
  Assert-Includes -Content $latest -Expected "version: $appVersion" -Label 'latest.yml version'
  Assert-Includes -Content $latest -Expected "path: $(Split-Path -Leaf $expectedInstaller)" -Label 'latest.yml installer path'
  Assert-Regex -Content $latest -Regex '(?m)^isAdminRightsRequired:\s*true\s*$' -Label 'top-level admin metadata'
  Assert-Regex -Content $latest -Regex '(?m)^    isAdminRightsRequired:\s*true\s*$' -Label 'file-entry admin metadata'
}

Write-Host 'C# package verification passed.'
