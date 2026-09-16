param([switch]$Sign, [switch]$Test)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$output = Join-Path $repo 'dist/keyboard-qualification'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$packs = Join-Path ${env:ProgramFiles(x86)} 'Reference Assemblies/Microsoft/Framework/.NETFramework'
if (-not (Test-Path $compiler)) { throw '.NET Framework compiler is required.' }
$assemblies = @('UIAutomationClient.dll', 'UIAutomationTypes.dll', 'WindowsBase.dll')
$folders = @()
if (Test-Path $packs) {
    foreach ($pack in (Get-ChildItem $packs -Directory | Sort-Object Name -Descending)) {
        $folders += $pack.FullName
        $folders += Join-Path $pack.FullName 'WPF'
    }
}
$folders += Join-Path (Split-Path -Parent $compiler) 'WPF'
$wpf = $folders | Where-Object {
    $candidate = $_
    @($assemblies | Where-Object { -not (Test-Path (Join-Path $candidate $_)) }).Count -eq 0
} | Select-Object -First 1
if (-not $wpf) { throw '.NET Framework UI Automation assemblies were not found in the targeting packs or framework WPF runtime.' }
$arguments = @('/nologo', '/target:winexe', '/platform:x64', '/r:System.Windows.Forms.dll', '/r:System.Drawing.dll', '/r:System.Core.dll', '/r:Accessibility.dll')
foreach ($assembly in $assemblies) {
    $arguments += '/r:' + (Join-Path $wpf $assembly)
}
$executable = Join-Path $output 'SwitchifyKeyboardQualification.exe'
$arguments += "/out:$executable"
$arguments += '/win32manifest:' + (Join-Path $repo 'tools/keyboard-qualification/WindowsProbe.manifest')
$arguments += Join-Path $repo 'tools/keyboard-qualification/WindowsProbe.cs'
& $compiler @arguments
if ($LASTEXITCODE -ne 0) { throw 'Keyboard qualification compilation failed.' }
if ($Test) {
    $testExecutable = Join-Path $output 'KeyboardQualificationTests.exe'
    $testArguments = @($arguments | Where-Object { $_ -notmatch '^/(out|target|win32manifest):' -and $_ -notlike '*.cs' })
    $testArguments += @('/target:exe', '/main:KeyboardQualificationTests', "/out:$testExecutable")
    $testArguments += Join-Path $repo 'tools/keyboard-qualification/WindowsProbe.cs'
    $testArguments += Join-Path $repo 'tools/keyboard-qualification/KeyboardQualificationTests.cs'
    & $compiler @testArguments
    if ($LASTEXITCODE -ne 0) { throw 'Keyboard qualification test compilation failed.' }
    & $testExecutable
    if ($LASTEXITCODE -ne 0) { throw 'Keyboard qualification tests failed.' }
}
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
