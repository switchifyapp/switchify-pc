$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$releaseDirectory = Join-Path $root 'src-tauri\target\release'
$main = Join-Path $releaseDirectory 'switchify-pc.exe'
$launcher = Join-Path $releaseDirectory 'switchify-pc-startup.exe'
$generatedInstaller = Join-Path $releaseDirectory 'nsis\x64\installer.nsi'
$installer = Get-ChildItem (Join-Path $releaseDirectory 'bundle\nsis') -Filter '*.exe' |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName

function Get-WindowsSdkTool([string]$Name) {
  $tool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Filter $Name -Recurse |
    Where-Object { $_.FullName -match "\\x64\\$([regex]::Escape($Name))$" } |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
  if (-not $tool) { throw "Windows SDK tool $Name was not found." }
  return $tool
}

function Assert-Manifest([string]$Executable, [string]$Level, [string]$UiAccess) {
  if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw "Missing executable: $Executable" }
  $temporary = Join-Path ([System.IO.Path]::GetTempPath()) "switchify-$([guid]::NewGuid().ToString('N')).manifest"
  try {
    & (Get-WindowsSdkTool 'mt.exe') -nologo "-inputresource:$Executable;#1" "-out:$temporary"
    if ($LASTEXITCODE -ne 0) { throw "Could not extract the manifest from $Executable." }
    [xml]$manifest = Get-Content -LiteralPath $temporary -Raw
    $node = $manifest.SelectSingleNode("//*[local-name()='requestedExecutionLevel']")
    if (-not $node -or $node.level -ne $Level -or $node.uiAccess -ne $UiAccess) {
      throw "Unexpected manifest for $Executable. Expected level=$Level uiAccess=$UiAccess."
    }
  } finally {
    Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
  }
}

function Assert-Signature([string]$Path) {
  $signature = Get-AuthenticodeSignature -LiteralPath $Path
  if ($signature.Status -eq 'Valid') { return }
  if ($env:SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE -eq '1' -and $signature.Status -eq 'NotSigned') {
    Write-Warning "Accepting unsigned development artifact: $Path"
    return
  }
  throw "Invalid Authenticode signature for $Path. Status: $($signature.Status)"
}

Assert-Manifest $main 'highestAvailable' 'true'
Assert-Manifest $launcher 'asInvoker' 'false'
Assert-Signature $main
Assert-Signature $launcher
Assert-Signature $installer

if (-not (Test-Path -LiteralPath $generatedInstaller -PathType Leaf)) {
  throw "Generated NSIS script is missing: $generatedInstaller"
}
$installerScript = Get-Content -LiteralPath $generatedInstaller -Raw
foreach ($expected in @(
  '!define INSTALLMODE "perMachine"',
  'switchify-pc-startup.exe',
  'installer-hooks.nsh'
)) {
  if (-not $installerScript.Contains($expected)) {
    throw "Generated NSIS script is missing: $expected"
  }
}

$configuration = Get-Content (Join-Path $root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
if ($configuration.bundle.windows.nsis.installMode -ne 'perMachine') {
  throw 'The Windows installer must use perMachine installation for UIAccess.'
}

Write-Output "Verified Windows UIAccess package: $installer"
