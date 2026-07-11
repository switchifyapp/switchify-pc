[CmdletBinding()]
param(
  [switch]$NoLaunch
)

$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'WinSigningTools.psm1') -Force -DisableNameChecking

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$projectPath = Join-Path $repositoryRoot 'src\SwitchifyPc.App\SwitchifyPc.App.csproj'
$sandboxDir = Join-Path $repositoryRoot 'dist\sandbox'
$appDir = Join-Path $sandboxDir 'app'
$configurationPath = Join-Path $sandboxDir 'Launch-Switchify-Test.wsb'
$appExecutable = Join-Path $appDir 'Switchify PC.exe'
$sandboxExecutable = Join-Path $env:WINDIR 'System32\WindowsSandbox.exe'

function Assert-SandboxManifest {
  param([Parameter(Mandatory = $true)][string]$ExecutablePath)

  $mt = Get-WindowsSdkTool -ToolName 'mt.exe'
  $temporaryManifest = Join-Path ([System.IO.Path]::GetTempPath()) ("switchify-sandbox-manifest-" + [Guid]::NewGuid().ToString('N') + '.xml')
  try {
    Invoke-ExternalTool -FilePath $mt -Arguments @('-nologo', "-inputresource:$ExecutablePath;#1", "-out:$temporaryManifest")
    [xml]$manifest = Get-Content -LiteralPath $temporaryManifest -Raw
    $executionLevel = $manifest.SelectSingleNode("//*[local-name()='requestedExecutionLevel']")
    if (-not $executionLevel -or $executionLevel.level -ne 'asInvoker' -or $executionLevel.uiAccess -ne 'false') {
      throw 'The sandbox executable must embed level=asInvoker and uiAccess=false.'
    }
  } finally {
    if (Test-Path -LiteralPath $temporaryManifest) {
      Remove-Item -LiteralPath $temporaryManifest -Force
    }
  }
}

New-Item -ItemType Directory -Path $sandboxDir -Force | Out-Null
if (Test-Path -LiteralPath $appDir) {
  $resolvedSandboxDir = [System.IO.Path]::GetFullPath($sandboxDir).TrimEnd('\')
  $resolvedAppDir = [System.IO.Path]::GetFullPath($appDir)
  if (-not $resolvedAppDir.StartsWith($resolvedSandboxDir + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to remove an app directory outside the sandbox output: $resolvedAppDir"
  }
  Remove-Item -LiteralPath $resolvedAppDir -Recurse -Force
}

& dotnet publish $projectPath -c Release -r win-x64 --self-contained true -p:SwitchifySandboxBuild=true -o $appDir
if ($LASTEXITCODE -ne 0) {
  throw "Sandbox publish failed with exit code $LASTEXITCODE."
}

if (-not (Test-Path -LiteralPath $appExecutable)) {
  throw "Published app executable was not found: $appExecutable"
}
Assert-SandboxManifest -ExecutablePath $appExecutable

$escapedHostFolder = [Security.SecurityElement]::Escape($appDir)
$configuration = @"
<Configuration>
  <Networking>Disable</Networking>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$escapedHostFolder</HostFolder>
      <SandboxFolder>C:\SwitchifyTest</SandboxFolder>
      <ReadOnly>true</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>cmd.exe /c start "" "C:\SwitchifyTest\Switchify PC.exe"</Command>
  </LogonCommand>
</Configuration>
"@

[System.IO.File]::WriteAllText($configurationPath, $configuration, [System.Text.UTF8Encoding]::new($false))
[xml]$parsedConfiguration = Get-Content -LiteralPath $configurationPath -Raw
if ($parsedConfiguration.Configuration.Networking -ne 'Disable' -or
    $parsedConfiguration.Configuration.MappedFolders.MappedFolder.ReadOnly -ne 'true' -or
    $parsedConfiguration.Configuration.MappedFolders.MappedFolder.SandboxFolder -ne 'C:\SwitchifyTest') {
  throw 'Generated Windows Sandbox configuration failed validation.'
}

Write-Host "Sandbox build and configuration created at $sandboxDir"
if ($NoLaunch) {
  Write-Host 'Launch skipped because -NoLaunch was specified.'
  return
}

if (-not (Test-Path -LiteralPath $sandboxExecutable)) {
  throw @"
Windows Sandbox is not available. From an elevated PowerShell prompt run:
Enable-WindowsOptionalFeature -FeatureName "Containers-DisposableClientVM" -All -Online
Restart Windows if requested, then run this script again.
"@
}

Start-Process -FilePath $sandboxExecutable -ArgumentList @($configurationPath)
Write-Host 'Windows Sandbox started. Closing it will discard its app data.'
