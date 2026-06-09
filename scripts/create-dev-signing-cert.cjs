const fs = require('node:fs');
const path = require('node:path');
const { runTool, resolveProjectPath } = require('./win-signing-tools.cjs');

const password = process.env.SWITCHIFY_DEV_CERT_PASSWORD;
if (!password) {
  throw new Error('SWITCHIFY_DEV_CERT_PASSWORD is required to create the dev signing certificate.');
}

const certDir = resolveProjectPath('.certs');
const pfxPath = process.env.SWITCHIFY_DEV_CERT_PFX || path.join(certDir, 'switchify-dev-code-signing.pfx');
const cerPath = path.join(path.dirname(pfxPath), 'switchify-dev-code-signing.cer');

fs.mkdirSync(path.dirname(pfxPath), { recursive: true });

const escapedPfxPath = escapePowerShellString(pfxPath);
const escapedCerPath = escapePowerShellString(cerPath);
const escapedPassword = escapePowerShellString(password);

const script = `
$password = ConvertTo-SecureString '${escapedPassword}' -AsPlainText -Force
$cert = New-SelfSignedCertificate -Subject 'CN=Switchify PC Dev Code Signing' -Type CodeSigningCert -CertStoreLocation 'Cert:\\CurrentUser\\My' -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -KeyExportPolicy Exportable
Export-PfxCertificate -Cert $cert -FilePath '${escapedPfxPath}' -Password $password | Out-Null
Export-Certificate -Cert $cert -FilePath '${escapedCerPath}' | Out-Null
Import-Certificate -FilePath '${escapedCerPath}' -CertStoreLocation 'Cert:\\CurrentUser\\Root' | Out-Null
Import-Certificate -FilePath '${escapedCerPath}' -CertStoreLocation 'Cert:\\CurrentUser\\TrustedPublisher' | Out-Null
Write-Host 'Created dev code-signing certificate.'
Write-Host 'PFX:' '${escapedPfxPath}'
Write-Host 'CER:' '${escapedCerPath}'
`.trim();

runTool('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', script]);

function escapePowerShellString(value) {
  return value.replace(/'/g, "''");
}
