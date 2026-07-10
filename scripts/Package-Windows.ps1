param(
  [switch]$StageOnly,
  [switch]$SkipSign,
  [string]$Configuration = 'Release',
  [string]$RuntimeIdentifier = 'win-x64'
)

$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'WinSigningTools.psm1') -Force -DisableNameChecking

$framework = 'net8.0-windows10.0.19041.0'
$appProjectPath = Resolve-ProjectPath 'src' 'SwitchifyPc.App' 'SwitchifyPc.App.csproj'
$launcherProjectPath = Resolve-ProjectPath 'src' 'SwitchifyPc.StartupLauncher' 'SwitchifyPc.StartupLauncher.csproj'
$version = Get-ProjectVersion -ProjectPath $appProjectPath
$publishDir = Resolve-ProjectPath 'src' 'SwitchifyPc.App' 'bin' $Configuration $framework $RuntimeIdentifier 'publish'
$launcherPublishDir = Resolve-ProjectPath 'src' 'SwitchifyPc.StartupLauncher' 'bin' $Configuration $framework $RuntimeIdentifier 'publish'
$distDir = Resolve-ProjectPath 'dist'
$stageDir = Join-Path $distDir 'win-unpacked'
$installerName = "Switchify-PC-Setup-$version-x64.exe"
$installerPath = Join-Path $distDir $installerName

function Reset-Directory {
  param([Parameter(Mandatory = $true)][string]$Path)

  if (Test-Path -LiteralPath $Path) {
    Remove-Item -LiteralPath $Path -Recurse -Force
  }
  New-Item -ItemType Directory -Path $Path -Force | Out-Null
}

function Reset-PackageArtifacts {
  New-Item -ItemType Directory -Path $distDir -Force | Out-Null

  $latest = Join-Path $distDir 'latest.yml'
  if (Test-Path -LiteralPath $latest) {
    Remove-Item -LiteralPath $latest -Force
  }

  Get-ChildItem -LiteralPath $distDir -File -Filter 'Switchify-PC-Setup-*-x64.exe' |
    Remove-Item -Force
}

function Copy-Directory {
  param(
    [Parameter(Mandatory = $true)][string]$From,
    [Parameter(Mandatory = $true)][string]$To
  )

  if (-not (Test-Path -LiteralPath $From -PathType Container)) {
    throw "Publish directory was not found: $From"
  }

  Copy-Item -Path (Join-Path $From '*') -Destination $To -Recurse -Force
}

function Assert-StagedApp {
  $executable = Join-Path $stageDir 'Switchify PC.exe'
  if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Staged C# app executable is missing: $executable"
  }

  $launcher = Join-Path $stageDir 'Switchify PC Startup.exe'
  if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) {
    throw "Staged startup launcher is missing: $launcher"
  }

  $markers = @(
    (Join-Path $stageDir 'resources'),
    (Join-Path $stageDir 'chrome_100_percent.pak'),
    (Join-Path $stageDir 'ffmpeg.dll')
  )

  foreach ($marker in $markers) {
    if (Test-Path -LiteralPath $marker) {
      throw "Staged C# app unexpectedly contains legacy runtime marker: $marker"
    }
  }
}

function Build-Installer {
  $makensis = Get-MakeNsisTool
  if (Test-Path -LiteralPath $installerPath) {
    Remove-Item -LiteralPath $installerPath -Force
  }

  Invoke-ExternalTool -FilePath $makensis -Arguments @(
    "/DVERSION=$version",
    "/DSOURCE_DIR=$stageDir",
    "/DOUTPUT_EXE=$installerPath",
    (Resolve-ProjectPath 'installer' 'SwitchifyPc.DotNet.nsi')
  )

  if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
    throw "NSIS did not create expected installer: $installerPath"
  }
}

function Write-LatestYml {
  $sha512 = Get-Sha512Base64 -Path $installerPath
  $size = (Get-Item -LiteralPath $installerPath).Length
  $releaseDate = [DateTimeOffset]::UtcNow.ToString('o')
  $latestPath = Join-Path $distDir 'latest.yml'
  $lines = @(
    "version: $version",
    'files:',
    "  - url: $installerName",
    "    sha512: $sha512",
    "    size: $size",
    '    isAdminRightsRequired: true',
    "path: $installerName",
    "sha512: $sha512",
    'isAdminRightsRequired: true',
    "releaseDate: '$releaseDate'",
    ''
  )

  Set-Content -LiteralPath $latestPath -Value $lines -Encoding UTF8
}

function Write-BuilderDebug {
  $builderDebugPath = Join-Path $distDir 'builder-debug.yml'
  $lines = @(
    "version: $version",
    'packager: dotnet-wpf-nsis',
    "stageDir: $(ConvertTo-YamlString $stageDir)",
    "publishDir: $(ConvertTo-YamlString $publishDir)",
    ''
  )

  Set-Content -LiteralPath $builderDebugPath -Value $lines -Encoding UTF8
}

function Get-Sha512Base64 {
  param([Parameter(Mandatory = $true)][string]$Path)

  $sha = [System.Security.Cryptography.SHA512]::Create()
  try {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
      return [Convert]::ToBase64String($sha.ComputeHash($stream))
    } finally {
      $stream.Dispose()
    }
  } finally {
    $sha.Dispose()
  }
}

function ConvertTo-YamlString {
  param([Parameter(Mandatory = $true)][string]$Value)

  return '"' + ($Value.Replace('\', '/').Replace('"', '\"')) + '"'
}

dotnet publish $appProjectPath -c $Configuration -r $RuntimeIdentifier --self-contained true
if ($LASTEXITCODE -ne 0) {
  throw "dotnet publish failed with exit code $LASTEXITCODE."
}

dotnet publish $launcherProjectPath -c $Configuration -r $RuntimeIdentifier --self-contained true -p:Version=$version
if ($LASTEXITCODE -ne 0) {
  throw "Startup launcher publish failed with exit code $LASTEXITCODE."
}

Reset-PackageArtifacts
Reset-Directory -Path $stageDir
Copy-Directory -From $publishDir -To $stageDir
$publishedLauncher = Join-Path $launcherPublishDir 'Switchify PC Startup.exe'
if (-not (Test-Path -LiteralPath $publishedLauncher -PathType Leaf)) {
  throw "Published startup launcher is missing: $publishedLauncher"
}
Copy-Item -LiteralPath $publishedLauncher -Destination $stageDir -Force
Assert-StagedApp
Write-BuilderDebug

$appExe = Join-Path $stageDir 'Switchify PC.exe'
$launcherExe = Join-Path $stageDir 'Switchify PC Startup.exe'
if (-not $SkipSign) {
  Invoke-SignFile -FilePath $appExe -RequireSigning $true
  Invoke-SignFile -FilePath $launcherExe -RequireSigning $true
}

if (-not $StageOnly) {
  Build-Installer
  if (-not $SkipSign) {
    Invoke-SignFile -FilePath $installerPath -RequireSigning $true
  }
  Write-LatestYml
}

if ($StageOnly) {
  Write-Host "Staged C# app in $stageDir"
} else {
  Write-Host "Packaged C# installer at $installerPath"
}
