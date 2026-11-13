param(
    [Parameter(Mandatory = $true)][string]$Python,
    [Parameter(Mandatory = $true)][string]$Model,
    [string]$Distro = 'Ubuntu',
    [switch]$Native
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$bridge = Join-Path $PSScriptRoot 'textslinger-spike.py'
if ($Native) {
    $Python = (Resolve-Path -LiteralPath $Python).Path
    $Model = (Resolve-Path -LiteralPath $Model).Path
} elseif (!$Python.StartsWith('/') -or !$Model.StartsWith('/')) {
    throw 'The WSL route requires absolute Linux paths for Python and Model'
}
$argv = @($Python, $bridge, '--model', $Model)
if (!$Native) {
    $linuxBridge = & wsl.exe --distribution $Distro --exec wslpath -a $bridge
    if ($LASTEXITCODE -ne 0) { throw 'Cannot translate the bridge path in WSL' }
    $argv = @('wsl.exe', '--distribution', $Distro, '--exec', $Python,
        $linuxBridge.Trim(), '--model', $Model)
}
$previous = $env:SWITCHIFY_TEXTSLINGER_COMMAND
try {
    $env:SWITCHIFY_TEXTSLINGER_COMMAND = ConvertTo-Json -Compress -InputObject $argv
    Push-Location $repo
    try {
        & cargo test --manifest-path src-tauri/Cargo.toml --lib textslinger -- --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'TextSlinger spike tests failed' }
    } finally { Pop-Location }
} finally {
    if ($null -eq $previous) { Remove-Item Env:SWITCHIFY_TEXTSLINGER_COMMAND -ErrorAction SilentlyContinue }
    else { $env:SWITCHIFY_TEXTSLINGER_COMMAND = $previous }
}
