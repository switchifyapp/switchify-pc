$ErrorActionPreference = 'Stop'

function Resolve-ProjectPath {
  param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Segments)

  $root = Resolve-Path (Join-Path $PSScriptRoot '..')
  $path = $root.Path
  if (-not $Segments -or $Segments.Count -eq 0) {
    return $path
  }

  foreach ($segment in $Segments) {
    $path = Join-Path $path $segment
  }

  return $path
}

function Get-WindowsSdkTool {
  param([Parameter(Mandatory = $true)][string]$ToolName)

  $overrideName = if ($ToolName.ToLowerInvariant() -eq 'mt.exe') { 'SWITCHIFY_MT_EXE' } else { 'SWITCHIFY_SIGNTOOL_EXE' }
  $override = [Environment]::GetEnvironmentVariable($overrideName)
  if ($override) {
    if (-not (Test-Path -LiteralPath $override -PathType Leaf)) {
      throw "$overrideName was configured but does not exist."
    }
    return $override
  }

  $sdkBinRoot = 'C:\Program Files (x86)\Windows Kits\10\bin'
  if (Test-Path -LiteralPath $sdkBinRoot -PathType Container) {
    $candidate = Get-ChildItem -LiteralPath $sdkBinRoot -Directory |
      Sort-Object Name -Descending |
      ForEach-Object {
        $toolPath = Join-Path $_.FullName "x64\$ToolName"
        if (Test-Path -LiteralPath $toolPath -PathType Leaf) { $toolPath }
      } |
      Select-Object -First 1

    if ($candidate) {
      return $candidate
    }
  }

  $whereResult = & cmd.exe /c "where $ToolName 2>NUL"
  if ($LASTEXITCODE -eq 0) {
    $first = $whereResult | Where-Object { $_ } | Select-Object -First 1
    if ($first) {
      return $first
    }
  }

  throw "Unable to find $ToolName. Install the Windows SDK or set $overrideName."
}

function Get-MakeNsisTool {
  $override = [Environment]::GetEnvironmentVariable('SWITCHIFY_MAKENSIS_EXE')
  if ($override) {
    if (-not (Test-Path -LiteralPath $override -PathType Leaf)) {
      throw 'SWITCHIFY_MAKENSIS_EXE does not exist.'
    }
    return $override
  }

  $candidates = @(
    'C:\Program Files (x86)\NSIS\makensis.exe',
    'C:\Program Files\NSIS\makensis.exe'
  )

  foreach ($candidate in $candidates) {
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
      return $candidate
    }
  }

  $whereResult = & cmd.exe /c 'where makensis.exe 2>NUL'
  if ($LASTEXITCODE -eq 0) {
    $first = $whereResult | Where-Object { $_ } | Select-Object -First 1
    if ($first) {
      return $first
    }
  }

  throw 'Unable to find makensis.exe. Install NSIS or set SWITCHIFY_MAKENSIS_EXE.'
}

function Invoke-ExternalTool {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [Parameter(Mandatory = $true)][string[]]$Arguments
  )

  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath failed with exit code $LASTEXITCODE."
  }
}

function Get-ProjectVersion {
  param([Parameter(Mandatory = $true)][string]$ProjectPath)

  [xml]$project = Get-Content -LiteralPath $ProjectPath
  $version = $project.Project.PropertyGroup |
    ForEach-Object { $_.Version } |
    Where-Object { $_ } |
    Select-Object -First 1

  if (-not $version) {
    throw "Could not read C# app <Version> from $ProjectPath."
  }

  return [string]$version
}

function Invoke-EmbedUiAccessManifest {
  param([Parameter(Mandatory = $true)][string]$ExecutablePath)

  $mt = Get-WindowsSdkTool -ToolName 'mt.exe'
  $manifestPath = Resolve-ProjectPath 'build' 'win-uiaccess.manifest'
  Invoke-ExternalTool -FilePath $mt -Arguments @('-nologo', '-manifest', $manifestPath, "-outputresource:$ExecutablePath;#1")
}

function Invoke-SignFile {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [bool]$RequireSigning = $true
  )

  $args = Get-SigningArguments -FilePath $FilePath -RequireSigning:$RequireSigning
  if (-not $args) {
    return
  }

  $signtool = Get-WindowsSdkTool -ToolName 'signtool.exe'
  Invoke-ExternalTool -FilePath $signtool -Arguments $args
}

function Get-SigningArguments {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [bool]$RequireSigning = $true
  )

  if (-not (Test-IsWindows)) {
    return $null
  }

  $mode = [Environment]::GetEnvironmentVariable('SWITCHIFY_SIGNING_MODE')
  if ($mode -eq 'certum-simplysign') {
    return Get-CertumSigningArguments -FilePath $FilePath -RequireSigning:$RequireSigning
  }

  if ($mode -eq 'azure') {
    return Get-AzureSigningArguments -FilePath $FilePath -RequireSigning:$RequireSigning
  }

  $devPfx = Get-DevPfxSigningArguments -FilePath $FilePath
  if ($devPfx) {
    return $devPfx
  }

  $devStore = Get-DevStoreSigningArguments -FilePath $FilePath
  if ($devStore) {
    return $devStore
  }

  $azure = Get-AzureSigningArguments -FilePath $FilePath -RequireSigning:$false
  if ($azure) {
    return $azure
  }

  if ([Environment]::GetEnvironmentVariable('SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE') -eq '1') {
    if ($RequireSigning) {
      Write-Warning 'Producing an unsigned uiAccess package because SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE=1.'
    }
    return $null
  }

  throw 'Signing is required for uiAccess packaging. Configure Certum SimplySign, a dev certificate, or Azure Artifact Signing.'
}

function Test-IsWindows {
  return [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
  )
}

function Get-CertumSigningArguments {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [bool]$RequireSigning
  )

  $thumbprint = Normalize-Thumbprint ([Environment]::GetEnvironmentVariable('SWITCHIFY_CERTUM_CERT_THUMBPRINT'))
  if (-not $thumbprint) {
    if ($RequireSigning) {
      throw 'Certum SimplySign signing requires SWITCHIFY_CERTUM_CERT_THUMBPRINT.'
    }
    return $null
  }

  $timestamp = [Environment]::GetEnvironmentVariable('SWITCHIFY_CERTUM_TIMESTAMP_URL')
  if (-not $timestamp) {
    $timestamp = 'http://time.certum.pl'
  }

  return @('sign', '/fd', 'SHA256', '/sha1', $thumbprint, '/tr', $timestamp, '/td', 'SHA256', $FilePath)
}

function Get-DevPfxSigningArguments {
  param([Parameter(Mandatory = $true)][string]$FilePath)

  $password = [Environment]::GetEnvironmentVariable('SWITCHIFY_DEV_CERT_PASSWORD')
  $pfxPath = [Environment]::GetEnvironmentVariable('SWITCHIFY_DEV_CERT_PFX')
  if (-not $pfxPath) {
    $pfxPath = Resolve-ProjectPath '.certs' 'switchify-dev-code-signing.pfx'
  }
  $thumbprint = Resolve-DevCertificateThumbprint -PfxPath $pfxPath

  if (-not $password -or -not (Test-Path -LiteralPath $pfxPath -PathType Leaf)) {
    return $null
  }

  $args = @('sign', '/fd', 'SHA256', '/f', $pfxPath, '/p', $password)
  if ($thumbprint) {
    $args += @('/sha1', $thumbprint)
  }
  if ([Environment]::GetEnvironmentVariable('SWITCHIFY_SIGN_SKIP_TIMESTAMP') -ne '1') {
    $args += @('/tr', 'http://timestamp.digicert.com', '/td', 'SHA256')
  }
  $args += $FilePath
  return $args
}

function Get-DevStoreSigningArguments {
  param([Parameter(Mandatory = $true)][string]$FilePath)

  $pfxPath = [Environment]::GetEnvironmentVariable('SWITCHIFY_DEV_CERT_PFX')
  if (-not $pfxPath) {
    $pfxPath = Resolve-ProjectPath '.certs' 'switchify-dev-code-signing.pfx'
  }
  $thumbprint = Resolve-DevCertificateThumbprint -PfxPath $pfxPath
  if (-not $thumbprint) {
    return $null
  }

  $args = @('sign', '/fd', 'SHA256', '/sha1', $thumbprint)
  if ([Environment]::GetEnvironmentVariable('SWITCHIFY_SIGN_SKIP_TIMESTAMP') -ne '1') {
    $args += @('/tr', 'http://timestamp.digicert.com', '/td', 'SHA256')
  }
  $args += $FilePath
  return $args
}

function Resolve-DevCertificateThumbprint {
  param([Parameter(Mandatory = $true)][string]$PfxPath)

  $explicit = Normalize-Thumbprint ([Environment]::GetEnvironmentVariable('SWITCHIFY_DEV_CERT_THUMBPRINT'))
  if ($explicit) {
    return $explicit
  }

  $thumbprintPath = Join-Path (Split-Path -Parent $PfxPath) 'switchify-dev-code-signing.thumbprint'
  if (-not (Test-Path -LiteralPath $thumbprintPath -PathType Leaf)) {
    return $null
  }

  return Normalize-Thumbprint (Get-Content -LiteralPath $thumbprintPath -Raw)
}

function Normalize-Thumbprint {
  param([AllowNull()][string]$Value)

  if (-not $Value) {
    return $null
  }

  $thumbprint = ($Value -replace '[^a-fA-F0-9]', '').ToUpperInvariant()
  if ($thumbprint.Length -eq 0) {
    return $null
  }
  return $thumbprint
}

function Get-AzureSigningArguments {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [bool]$RequireSigning
  )

  $dlib = [Environment]::GetEnvironmentVariable('SWITCHIFY_AZURE_SIGNING_DLIB')
  $metadata = [Environment]::GetEnvironmentVariable('SWITCHIFY_AZURE_SIGNING_METADATA')
  if (-not $dlib -or -not $metadata) {
    if ($RequireSigning) {
      throw 'Azure Artifact Signing requires SWITCHIFY_AZURE_SIGNING_DLIB and SWITCHIFY_AZURE_SIGNING_METADATA.'
    }
    return $null
  }

  return @(
    'sign',
    '/fd',
    'SHA256',
    '/tr',
    'http://timestamp.acs.microsoft.com',
    '/td',
    'SHA256',
    '/dlib',
    $dlib,
    '/dmdf',
    $metadata,
    $FilePath
  )
}
