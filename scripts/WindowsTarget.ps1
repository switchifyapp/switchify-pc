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

# Every ARM64 build needs clang for ring. A cross build from x64 also needs
# the MSVC ARM64 build tools. Visual Studio's Clang component
# does not add itself to PATH.
function Initialize-WindowsCrossTools($WindowsTarget) {
  if ($WindowsTarget.Triple -ne 'aarch64-pc-windows-msvc') { return }
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  if (-not (Test-Path -LiteralPath $vswhere)) { throw 'vswhere.exe was not found. Install Visual Studio with C++ build tools.' }
  if ($WindowsTarget.Cross) {
    $arm64Tools = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.ARM64 -property installationPath
    if (-not $arm64Tools) {
      throw 'The MSVC ARM64 build tools are missing. Add the Visual Studio component Microsoft.VisualStudio.Component.VC.Tools.ARM64.'
    }
  }
  if (Get-Command clang.exe -ErrorAction SilentlyContinue) { return }
  $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Llvm.Clang -property installationPath |
    Select-Object -First 1
  # Visual Studio installs LLVM for each host CPU. Use the one this host runs:
  # a cross build here is always from x64, a native build runs on ARM64.
  $hostFolder = if ($WindowsTarget.Cross) { 'x64' } else { 'ARM64' }
  $clang = if ($vs) { Join-Path $vs "VC\Tools\Llvm\$hostFolder\bin" }
  if (-not $clang -or -not (Test-Path -LiteralPath (Join-Path $clang 'clang.exe'))) {
    throw 'clang.exe is missing. Add the Visual Studio component Microsoft.VisualStudio.Component.VC.Llvm.Clang.'
  }
  $env:PATH = "$clang;$env:PATH"
}
