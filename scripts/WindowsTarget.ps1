# Shared by the Windows package and verify scripts. A build for the host
# target keeps Cargo's default target\release output; a cross build uses
# target\<triple>\release.

$script:WindowsTargets = @{
  'x86_64-pc-windows-msvc' = @{ Nsis = 'x64'; Accelerated = $true }
  'aarch64-pc-windows-msvc' = @{ Nsis = 'arm64'; Accelerated = $false }
}

function Get-RustHostTarget([string]$Rustc) {
  $hostLine = & $Rustc -vV | Where-Object { $_ -like 'host: *' } | Select-Object -First 1
  if (-not $hostLine) { throw 'Could not read the Rust host target from rustc -vV.' }
  return $hostLine.Substring(6).Trim()
}

function Resolve-WindowsTarget([string]$Target, [string]$Rustc) {
  $hostTarget = Get-RustHostTarget $Rustc
  if (-not $Target) { $Target = $hostTarget }
  if (-not $script:WindowsTargets.ContainsKey($Target)) {
    throw "Unsupported Windows target: $Target. Use x86_64-pc-windows-msvc or aarch64-pc-windows-msvc."
  }
  $info = $script:WindowsTargets[$Target]
  return [pscustomobject]@{
    Triple = $Target
    Cross = $Target -ne $hostTarget
    ReleaseRelative = if ($Target -eq $hostTarget) { 'release' } else { "$Target\release" }
    Nsis = $info.Nsis
    Accelerated = $info.Accelerated
  }
}

function Find-Rustc {
  $rustc = Get-Command rustc.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
  if (-not $rustc) { $rustc = Join-Path $env:USERPROFILE '.cargo\bin\rustc.exe' }
  if (-not (Test-Path -LiteralPath $rustc)) { throw 'rustc.exe was not found.' }
  return $rustc
}

# Every ARM64 build needs clang for ring, and the MSVC ARM64 build tools
# (installed by default on an ARM64 host). Visual Studio's Clang component
# does not add itself to PATH.
function Initialize-WindowsCrossTools($WindowsTarget) {
  if ($WindowsTarget.Triple -ne 'aarch64-pc-windows-msvc') { return }
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  if (-not (Test-Path -LiteralPath $vswhere)) { throw 'vswhere.exe was not found. Install Visual Studio with C++ build tools.' }
  # An ARM64 host has native ARM64 tools, so only a cross build requires the component.
  $requires = if ($WindowsTarget.Cross) { @('-requires', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64') } else { @() }
  $vs = & $vswhere -latest -products * @requires -property installationPath | Select-Object -First 1
  if (-not $vs) {
    throw 'The MSVC ARM64 build tools are missing. Add the Visual Studio component Microsoft.VisualStudio.Component.VC.Tools.ARM64.'
  }
  if (-not (Get-Command clang.exe -ErrorAction SilentlyContinue)) {
    $clang = @('ARM64', 'x64') | ForEach-Object { Join-Path $vs "VC\Tools\Llvm\$_\bin" } |
      Where-Object { Test-Path -LiteralPath (Join-Path $_ 'clang.exe') } | Select-Object -First 1
    if (-not $clang) {
      throw 'clang.exe is missing. Add the Visual Studio component Microsoft.VisualStudio.Component.VC.Llvm.Clang.'
    }
    $env:PATH = "$clang;$env:PATH"
  }
}
