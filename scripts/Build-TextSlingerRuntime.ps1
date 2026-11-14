# Builds a self-contained Windows x64 TextSlinger n-gram runtime for local
# testing: standalone Python 3.10, numpy, regex, KenLM (MAX_ORDER=12),
# textslinger from PyPI, and the binary tiny 12-gram model. No PyTorch.
#
# textslinger 0.4.0 imports PyTorch on load. Until kdv123/textslinger#2 is
# released, textslinger/lazy-torch-imports.patch is applied to the installed
# package. Remove the patch step when a release includes that change.
param(
    [string]$Output,
    [string]$Uv,
    [string]$CMakeGenerator = 'Visual Studio 18 2026'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$cache = Join-Path $repo '.cache'
if (!$Output) { $Output = Join-Path $cache 'textslinger-runtime' }
# GIT_CEILING_DIRECTORIES below needs an absolute path.
$Output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Output)
$work = Join-Path $cache 'textslinger-runtime-build'
$patch = Join-Path $PSScriptRoot 'textslinger/lazy-torch-imports.patch'

$PythonVersion = '3.10.22'
# Exact files from PyPI, checked by hash.
$Packages = @(
    'numpy==1.26.4 --hash=sha256:b97fe8060236edf3662adfc2c633f56a08ae30560c56310562cb4f95500022d5',
    'regex==2026.9.29 --hash=sha256:23ae6fdad9e63e54038f5ef78aba2933faca61e24d432786589e737bc5522ebb',
    'textslinger==0.4.0 --hash=sha256:d33238e58c2f18403f5b777bd4b8f1148638a6e4c4d6a1094f3b487d01f46f22'
)
$KenLM = 'kenlm==0.2.0 --hash=sha256:c2dc1dc09d3c150e6a1777ef0fd5cac3688e1dc1cc13e41472d0284e5e88ac7f'
$BuildTools = @('cmake==4.4.4', 'setuptools==84.2.1', 'wheel==0.48.0', 'pip==26.3')
$ModelUrl = 'https://osf.io/download/zyj25/'
$ModelSha256 = 'd17ea73ef2edf180b55e0e636524f32c76fb6ab110c69ed196432438b1baf9c3'
$ModelName = 'lm_dec19_char_tiny_12gram.kenlm'
# Parts of the standard library the bridge never imports.
$TrimLib = @('test', 'tkinter', 'idlelib', 'ensurepip', 'lib2to3', 'turtledemo', 'pydoc_data',
    'unittest/test', 'ctypes/test', 'sqlite3/test', 'distutils/tests', 'site-packages')
$TrimDlls = @('_tkinter.pyd', 'tcl86t.dll', 'tk86t.dll')

function Invoke-Checked([string]$File, [string[]]$Arguments) {
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File failed with exit code $LASTEXITCODE" }
}

if (!$Uv) {
    $found = Get-Command uv -ErrorAction SilentlyContinue
    $Uv = if ($found) { $found.Source } else { Join-Path $env:LOCALAPPDATA 'SwitchifyTextSlingerSpike/uv.exe' }
}
if (!(Test-Path -LiteralPath $Uv)) { throw "uv was not found. Install uv or pass -Uv." }
if (!(Test-Path -LiteralPath $patch)) { throw "Missing $patch" }
New-Item -ItemType Directory -Path $work -Force | Out-Null

# Standalone CPython, managed by uv.
Invoke-Checked $Uv @('python', 'install', $PythonVersion)
# Only uv's own standalone build, never an active venv or a system Python.
$found = & $Uv python find --managed-python --no-project $PythonVersion
if ($LASTEXITCODE -ne 0 -or !$found) { throw "uv could not find its managed Python $PythonVersion" }
$basePython = ($found | Select-Object -Last 1).Trim()
if (!(Test-Path -LiteralPath $basePython)) { throw "Python $PythonVersion was not found at $basePython" }
$base = Split-Path -Parent $basePython

# KenLM has no Windows wheel and must be built with MAX_ORDER=12 for this
# model. Build it once into a cached wheel.
$wheels = Join-Path $work 'wheels'
$kenlmWheel = Get-ChildItem -Path $wheels -Filter 'kenlm-0.2.0-*.whl' -ErrorAction SilentlyContinue | Select-Object -First 1
if (!$kenlmWheel) {
    $buildVenv = Join-Path $work 'build-venv'
    if (!(Test-Path -LiteralPath (Join-Path $buildVenv 'Scripts/python.exe'))) {
        Invoke-Checked $Uv @('venv', '--python', $basePython, $buildVenv)
    }
    $buildPython = Join-Path $buildVenv 'Scripts/python.exe'
    Invoke-Checked $Uv (@('pip', 'install', '--python', $buildPython) + $BuildTools)
    $kenlmRequirements = Join-Path $work 'kenlm-requirements.txt'
    Set-Content -LiteralPath $kenlmRequirements -Value $KenLM -Encoding ascii
    $saved = @{ PATH = $env:PATH; MAX_ORDER = $env:MAX_ORDER; CMAKE_GENERATOR = $env:CMAKE_GENERATOR; CMAKE_POLICY_VERSION_MINIMUM = $env:CMAKE_POLICY_VERSION_MINIMUM }
    try {
        $env:PATH = (Join-Path $buildVenv 'Scripts') + ';' + $env:PATH
        $env:MAX_ORDER = '12'
        $env:CMAKE_GENERATOR = $CMakeGenerator
        $env:CMAKE_POLICY_VERSION_MINIMUM = '3.5'
        Invoke-Checked $buildPython @('-m', 'pip', 'wheel', '--no-cache-dir', '--no-build-isolation', '--no-deps',
            '--no-binary', 'kenlm', '--require-hashes', '-w', $wheels, '-r', $kenlmRequirements)
    } finally {
        foreach ($name in $saved.Keys) {
            if ($null -eq $saved[$name]) { Remove-Item "Env:$name" -ErrorAction SilentlyContinue }
            else { Set-Item "Env:$name" $saved[$name] }
        }
    }
    $kenlmWheel = Get-ChildItem -Path $wheels -Filter 'kenlm-0.2.0-*.whl' | Select-Object -First 1
    if (!$kenlmWheel) { throw 'The KenLM wheel was not built' }
}

# Download and verify the model once.
$model = Join-Path $work $ModelName
if (!(Test-Path -LiteralPath $model) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $model).Hash -ne $ModelSha256) {
    $compressed = "$model.gz"
    Invoke-WebRequest -UseBasicParsing -Uri $ModelUrl -OutFile $compressed
    $source = [IO.File]::OpenRead($compressed)
    try {
        $gzip = New-Object IO.Compression.GZipStream($source, [IO.Compression.CompressionMode]::Decompress)
        $target = [IO.File]::Create($model)
        try { $gzip.CopyTo($target) } finally { $target.Dispose(); $gzip.Dispose() }
    } finally { $source.Dispose() }
    Remove-Item -LiteralPath $compressed
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $model).Hash
    if ($hash -ne $ModelSha256) {
        Remove-Item -LiteralPath $model
        throw "The downloaded model has SHA-256 $hash, expected $ModelSha256"
    }
}

# Assemble the runtime from scratch so nothing stale is left behind.
$marker = Join-Path $Output '.switchify-textslinger-runtime'
if (Test-Path -LiteralPath $Output) {
    if (!(Test-Path -LiteralPath $marker)) { throw "$Output exists and was not made by this script" }
    Remove-Item -LiteralPath $Output -Recurse -Force
}
New-Item -ItemType Directory -Path $Output | Out-Null
Set-Content -LiteralPath $marker -Value '' -Encoding ascii
Get-ChildItem -LiteralPath $base -File | Where-Object { $_.Name -ne 'pythonw.exe' } | Copy-Item -Destination $Output
Copy-Item -LiteralPath (Join-Path $base 'DLLs') -Destination $Output -Recurse
Copy-Item -LiteralPath (Join-Path $base 'Lib') -Destination $Output -Recurse
foreach ($name in $TrimDlls) { Remove-Item -LiteralPath (Join-Path $Output "DLLs/$name") -ErrorAction SilentlyContinue }
foreach ($name in $TrimLib) { Remove-Item -LiteralPath (Join-Path $Output "Lib/$name") -Recurse -Force -ErrorAction SilentlyContinue }
$sitePackages = Join-Path $Output 'Lib/site-packages'
New-Item -ItemType Directory -Path $sitePackages | Out-Null

$requirements = Join-Path $work 'requirements.txt'
Set-Content -LiteralPath $requirements -Value $Packages -Encoding ascii
Invoke-Checked $Uv @('pip', 'install', '--python', $basePython, '--no-deps', '--require-hashes', '--target', $sitePackages, '-r', $requirements)
# The KenLM wheel is built locally from the hash-checked source above.
Invoke-Checked $Uv @('pip', 'install', '--python', $basePython, '--no-deps', '--target', $sitePackages, $kenlmWheel.FullName)

# Temporary until kdv123/textslinger#2 is released. Inside a repository, git
# apply resolves paths from its root and skips ignored files, so stop it from
# finding this checkout.
$savedCeiling = $env:GIT_CEILING_DIRECTORIES
Push-Location $sitePackages
try {
    $env:GIT_CEILING_DIRECTORIES = $Output
    Invoke-Checked git @('apply', '--whitespace=nowarn', $patch)
} finally {
    Pop-Location
    if ($null -eq $savedCeiling) { Remove-Item Env:GIT_CEILING_DIRECTORIES -ErrorAction SilentlyContinue }
    else { $env:GIT_CEILING_DIRECTORIES = $savedCeiling }
}
if (!(Select-String -LiteralPath (Join-Path $sitePackages 'textslinger/helpers.py') -Pattern '_precision_to_torch' -Quiet)) {
    throw 'The lazy import patch was not applied'
}

Get-ChildItem -LiteralPath $Output -Recurse -Directory -Filter '__pycache__' | Remove-Item -Recurse -Force
New-Item -ItemType Directory -Path (Join-Path $Output 'model') | Out-Null
Copy-Item -LiteralPath $model -Destination (Join-Path $Output "model/$ModelName")

# The runtime must load the model and predict without PyTorch present.
$runtimePython = Join-Path $Output 'python.exe'
$check = @'
import sys
from textslinger import NGramLanguageModel
alphabet = tuple("!',. ?abcdefghijklmnopqrstuvwxyz")
model = NGramLanguageModel(sys.argv[1], model_alphabet=alphabet)
words = model.predict_words(left_context="please open the ",
    word_spelling_characters=tuple("abcdefghijklmnopqrstuvwxyz'"), word_ending_characters=(" ",), nbest=5).top(5)
assert words, "no predictions"
assert not any(name.split(".")[0] == "torch" for name in sys.modules), "torch was imported"
'@
# Windows PowerShell drops quotes in native arguments, so run it from a file.
$checkFile = Join-Path $work 'check.py'
Set-Content -LiteralPath $checkFile -Value $check -Encoding ascii
Invoke-Checked $runtimePython @('-I', $checkFile, (Join-Path $Output "model/$ModelName"))

$size = (Get-ChildItem -LiteralPath $Output -Recurse -File | Measure-Object -Property Length -Sum).Sum
Write-Output ('TextSlinger runtime ready at {0} ({1:N0} MB)' -f $Output, ($size / 1MB))
Write-Output ('Python: {0}' -f $runtimePython)
Write-Output ('Model:  {0}' -f (Join-Path $Output "model/$ModelName"))
