$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'WinSigningTools.psm1') -Force -DisableNameChecking

$distDir = Resolve-ProjectPath 'dist'
$stageDir = Join-Path $distDir 'win-unpacked'
$appExe = Join-Path $stageDir 'Switchify PC.exe'
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
  Assert-Includes -Content $content -Expected 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchify PC' -Label 'installer uninstall registry key'
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
Assert-Exists -Path $builderDebug -Label 'C# builder debug metadata'
Assert-InstallerScript
Assert-NoLegacyRuntime

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
