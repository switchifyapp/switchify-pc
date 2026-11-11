param(
  [switch]$SkipSign,
  [string]$TauriConfig = 'src-tauri/tauri.windows-uiaccess.conf.json',
  # Rust target triple. Defaults to the host: x86_64-pc-windows-msvc or aarch64-pc-windows-msvc.
  [string]$Target
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'WindowsTarget.ps1')
$root = Split-Path -Parent $PSScriptRoot
$windowsTarget = Resolve-WindowsTarget $Target (Find-Rustc)
$triple = $windowsTarget.Triple
$launcherManifest = Join-Path $root 'src-tauri\startup-launcher\Cargo.toml'
$launcherSource = Join-Path $root "src-tauri\startup-launcher\target\$triple\release\switchify-pc-startup.exe"
$sidecarDirectory = Join-Path $root 'src-tauri\binaries'
$sidecar = Join-Path $sidecarDirectory "switchify-pc-startup-$triple.exe"
$main = Join-Path $root "src-tauri\target\$($windowsTarget.ReleaseRelative)\switchify-pc.exe"
$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
if (-not $cargo) { $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
$npm = Get-Command npm.cmd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
if (-not $npm) { $npm = 'C:\Program Files\nodejs\npm.cmd' }
if (-not (Test-Path -LiteralPath $cargo)) { throw 'cargo.exe was not found.' }
if (-not (Test-Path -LiteralPath $npm)) { throw 'npm.cmd was not found.' }
$env:PATH = "$(Split-Path -Parent $cargo);$(Split-Path -Parent $npm);$env:PATH"
Initialize-WindowsCrossTools $windowsTarget

$tauriArgs = @('run', 'tauri', 'build', '--', '--bundles', 'nsis', '--config', $TauriConfig)
if (-not $windowsTarget.Accelerated) {
  # Applied last so it replaces the externalBin list that names the AVX2 worker.
  $tauriArgs += @('--config', 'src-tauri/tauri.windows-arm64.conf.json')
}
if ($windowsTarget.Cross) { $tauriArgs += @('--target', $triple) }

Push-Location $root
try {
  & $cargo build --locked --manifest-path $launcherManifest --release --target $triple
  if ($LASTEXITCODE -ne 0) { throw "Startup launcher build failed with exit code $LASTEXITCODE." }

  New-Item -ItemType Directory -Path $sidecarDirectory -Force | Out-Null
  Copy-Item -LiteralPath $launcherSource -Destination $sidecar -Force

  if ($SkipSign) {
    $env:SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE = '1'
  } else {
    & (Join-Path $PSScriptRoot 'Sign-Windows.ps1') $sidecar
  }

  & $npm @tauriArgs
  if ($LASTEXITCODE -ne 0) { throw "Tauri package build failed with exit code $LASTEXITCODE." }

  if (-not $SkipSign) {
    & (Join-Path $PSScriptRoot 'Sign-Windows.ps1') $main
  }
} finally {
  Pop-Location
}
