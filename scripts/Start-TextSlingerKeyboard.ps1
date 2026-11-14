param(
    [string]$Python = (Join-Path $env:LOCALAPPDATA 'SwitchifyTextSlingerSpike/venv/Scripts/python.exe'),
    [string]$Model,
    [switch]$Dev,
    [switch]$SkipBuild,
    [switch]$NoLaunch,
    # Use the runtime from Build-TextSlingerRuntime.ps1 instead of the spike venv.
    [switch]$Runtime
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if ($Runtime) {
    $runtimeDirectory = Join-Path $repo '.cache/textslinger-runtime'
    if (!$PSBoundParameters.ContainsKey('Python')) { $Python = Join-Path $runtimeDirectory 'python.exe' }
    if (!$Model) { $Model = Join-Path $runtimeDirectory 'model/lm_dec19_char_tiny_12gram.kenlm' }
    if (!(Test-Path -LiteralPath $Python)) { throw 'Run scripts/Build-TextSlingerRuntime.ps1 first' }
}
if (!$Model) { $Model = Join-Path (Split-Path -Parent $repo) 'textslinger/assets/lm_dec19_char_tiny_12gram.arpa' }
$Python = (Resolve-Path -LiteralPath $Python).Path
# The built runtime runs isolated, as its build check does, so PYTHONPATH,
# PYTHONHOME and user site-packages cannot replace its patched packages.
$pythonArguments = if ($Runtime) { @($Python, '-I') } else { @($Python) }
$Model = (Resolve-Path -LiteralPath $Model).Path
if (Get-Process -Name switchify-pc -ErrorAction SilentlyContinue) {
    throw 'Quit Switchify PC from its tray menu before building or opening this trial'
}
$trial = Join-Path $repo '.cache/textslinger-keyboard'
New-Item -ItemType Directory -Path $trial -Force | Out-Null
$bridge = Join-Path $trial 'bridge.py'
$appDirectory = Join-Path $trial 'app'
$binary = Join-Path $appDirectory 'switchify-pc.exe'
Push-Location $repo
try {
    if ($Dev) {
        if ($SkipBuild -or $NoLaunch) { throw '-Dev cannot be combined with -SkipBuild or -NoLaunch' }
        $previous = $env:SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND
        try {
            $env:SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND = ConvertTo-Json -Compress -InputObject @(
                $pythonArguments + @((Join-Path $PSScriptRoot 'textslinger-spike.py'), '--model', $Model, '--keyboard-trial')
            )
            & npm.cmd run tauri -- dev --features textslinger-trial
            if ($LASTEXITCODE -ne 0) { throw 'TextSlinger keyboard dev exited with an error' }
        } finally {
            if ($null -eq $previous) { Remove-Item Env:SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND -ErrorAction SilentlyContinue }
            else { $env:SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND = $previous }
        }
        return
    }
    if (!$SkipBuild) {
        & npm.cmd run tauri -- build --debug --no-bundle --features textslinger-trial,tauri/custom-protocol
        if ($LASTEXITCODE -ne 0) { throw 'TextSlinger keyboard build failed' }
        New-Item -ItemType Directory -Path $appDirectory -Force | Out-Null
        $builtBinary = Join-Path $repo 'src-tauri/target/debug/switchify-pc.exe'
        Copy-Item -LiteralPath $builtBinary -Destination $binary -Force
        Get-ChildItem -LiteralPath (Split-Path -Parent $builtBinary) -Filter '*.dll' -File | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination $appDirectory -Force
        }
    }
    # Always refresh the bridge so -SkipBuild cannot launch an older copy.
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'textslinger-spike.py') -Destination $bridge -Force
    if (!(Test-Path -LiteralPath $binary) -or !(Test-Path -LiteralPath $bridge)) { throw 'The trial executable has not been built' }
    if (!$NoLaunch) {
        $previous = $env:SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND
        try {
            $env:SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND = ConvertTo-Json -Compress -InputObject @(
                $pythonArguments + @($bridge, '--model', $Model, '--keyboard-trial')
            )
            Start-Process -FilePath $binary -WorkingDirectory $repo -WindowStyle Normal
        } finally {
            if ($null -eq $previous) { Remove-Item Env:SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND -ErrorAction SilentlyContinue }
            else { $env:SWITCHIFY_TEXTSLINGER_KEYBOARD_COMMAND = $previous }
        }
        Write-Output 'Switchify PC is running with local TextSlinger word suggestions. Open your scanning keyboard as usual.'
        Write-Output 'The row stays empty while the model loads. Five word slots are used; phrase slots stay empty.'
        Write-Output 'Quit this app and open your installed Switchify PC to return to normal predictions.'
    }
} finally { Pop-Location }
