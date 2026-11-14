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
$work = Join-Path $cache 'textslinger-runtime-build'
$patch = Join-Path $PSScriptRoot 'textslinger/lazy-torch-imports.patch'

$PythonVersion = '3.10.22'
$Packages = @('numpy==1.26.4', 'regex==2026.9.29', 'textslinger==0.4.0')
$KenLM = 'kenlm==0.2.0'
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
$basePython = (& $Uv python find $PythonVersion | Select-Object -Last 1).Trim()
if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $basePython)) { throw "Python $PythonVersion was not found" }
$base = Split-Path -Parent $basePython

# KenLM has no Windows wheel and must be built with MAX_ORDER=12 for this
# model. Build it once into a cached wheel.
$wheels = Join-Path $work 'wheels'
$kenlmWheel = Get-ChildItem -Path $wheels -Filter 'kenlm-0.2.0-*.whl' -ErrorAction SilentlyContinue | Select-Object -First 1
if (!$kenlmWheel) {
    $buildVenv = Join-Path $work 'build-venv'
    if (!(Test-Path -LiteralPath (Join-Path $buildVenv 'Scripts/python.exe'))) {
        Invoke-Checked $Uv @('venv', '--python', $PythonVersion, $buildVenv)
    }
    $buildPython = Join-Path $buildVenv 'Scripts/python.exe'
    Invoke-Checked $Uv @('pip', 'install', '--python', $buildPython, 'cmake==4.4.4', 'setuptools>=77', 'wheel', 'pip')
    $saved = @{ PATH = $env:PATH; MAX_ORDER = $env:MAX_ORDER; CMAKE_GENERATOR = $env:CMAKE_GENERATOR; CMAKE_POLICY_VERSION_MINIMUM = $env:CMAKE_POLICY_VERSION_MINIMUM }
    try {
        $env:PATH = (Join-Path $buildVenv 'Scripts') + ';' + $env:PATH
        $env:MAX_ORDER = '12'
        $env:CMAKE_GENERATOR = $CMakeGenerator
        $env:CMAKE_POLICY_VERSION_MINIMUM = '3.5'
        Invoke-Checked $buildPython @('-m', 'pip', 'wheel', '--no-cache-dir', '--no-build-isolation', '--no-deps',
            '--no-binary', 'kenlm', '-w', $wheels, $KenLM)
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

Invoke-Checked $Uv (@('pip', 'install', '--python', $basePython, '--no-deps', '--target', $sitePackages) + $Packages + @($kenlmWheel.FullName))

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
