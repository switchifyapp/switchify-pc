$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'WinSigningTools.psm1') -Force -DisableNameChecking

$latestPath = Resolve-ProjectPath 'dist' 'latest.yml'
if (-not (Test-Path -LiteralPath $latestPath -PathType Leaf)) {
  throw "latest.yml was not found: $latestPath"
}

$content = Get-Content -LiteralPath $latestPath -Raw
if ($content -notmatch '(?m)^isAdminRightsRequired:\s*true\s*$') {
  throw 'latest.yml is missing top-level isAdminRightsRequired: true.'
}

if ($content -notmatch '(?m)^    isAdminRightsRequired:\s*true\s*$') {
  throw 'latest.yml is missing installer file entry isAdminRightsRequired: true.'
}

Write-Host 'latest.yml admin-rights metadata: present'
