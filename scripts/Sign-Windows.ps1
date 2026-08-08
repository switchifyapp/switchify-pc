param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$FilePath
)

$ErrorActionPreference = 'Stop'

if ($env:SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE -eq '1') {
  Write-Warning "Leaving development artifact unsigned: $FilePath"
  exit 0
}

$thumbprint = ($env:SWITCHIFY_CERTUM_CERT_THUMBPRINT -replace '\s', '').ToUpperInvariant()
if (-not $thumbprint) {
  throw 'SWITCHIFY_CERTUM_CERT_THUMBPRINT must identify the SimplySign code-signing certificate.'
}

$certificate = Get-ChildItem Cert:\CurrentUser\My | Where-Object {
  $_.Thumbprint -eq $thumbprint -and $_.EnhancedKeyUsageList.ObjectId -contains '1.3.6.1.5.5.7.3.3'
} | Select-Object -First 1
if (-not $certificate) {
  throw "The code-signing certificate $thumbprint is not available. Sign in to SimplySign Desktop first."
}

$signTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Filter signtool.exe -Recurse |
  Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
  Sort-Object FullName -Descending |
  Select-Object -First 1 -ExpandProperty FullName
if (-not $signTool) {
  throw 'Windows SDK signtool.exe was not found.'
}

$timestampUrl = if ($env:SWITCHIFY_CERTUM_TIMESTAMP_URL) {
  $env:SWITCHIFY_CERTUM_TIMESTAMP_URL
} else {
  'http://time.certum.pl'
}

& $signTool sign /sha1 $thumbprint /fd SHA256 /tr $timestampUrl /td SHA256 $FilePath
if ($LASTEXITCODE -ne 0) {
  throw "signtool failed for $FilePath with exit code $LASTEXITCODE."
}
