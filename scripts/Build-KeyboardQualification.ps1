param([switch]$Sign)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$output = Join-Path $repo 'dist/keyboard-qualification'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$packs = Join-Path ${env:ProgramFiles(x86)} 'Reference Assemblies/Microsoft/Framework/.NETFramework'
$references = Get-ChildItem $packs -Directory | Sort-Object Name -Descending | Select-Object -First 1
if (-not $references -or -not (Test-Path $compiler)) {
    throw '.NET Framework compiler and reference assemblies are required.'
}
$wpf = $references.FullName
if (-not (Test-Path (Join-Path $wpf 'UIAutomationClient.dll'))) {
    $wpf = Join-Path $wpf 'WPF'
}
$assemblies = @('UIAutomationClient.dll', 'UIAutomationTypes.dll', 'WindowsBase.dll')
$arguments = @('/nologo', '/target:winexe', '/platform:x64', '/r:System.Windows.Forms.dll', '/r:System.Drawing.dll', '/r:System.Core.dll')
foreach ($assembly in $assemblies) {
    $path = Join-Path $wpf $assembly
    if (-not (Test-Path $path)) { throw "Missing reference assembly: $assembly" }
    $arguments += "/r:$path"
}
$executable = Join-Path $output 'SwitchifyKeyboardQualification.exe'
$arguments += "/out:$executable"
$arguments += '/win32manifest:' + (Join-Path $repo 'tools/keyboard-qualification/WindowsProbe.manifest')
$arguments += Join-Path $repo 'tools/keyboard-qualification/WindowsProbe.cs'
& $compiler @arguments
if ($LASTEXITCODE -ne 0) { throw 'Keyboard qualification compilation failed.' }
if ($Sign) {
    if ($env:SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE -eq '1') {
        throw 'Disable the unsigned development override before requesting a signed probe.'
    }
    & (Join-Path $PSScriptRoot 'Sign-Windows.ps1') $executable
    if ((Get-AuthenticodeSignature $executable).Status -ne 'Valid') {
        throw 'Keyboard qualification signature verification failed.'
    }
}
Write-Output $executable
