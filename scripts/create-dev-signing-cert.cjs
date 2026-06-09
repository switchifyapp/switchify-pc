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
const rootCerPath = path.join(path.dirname(pfxPath), 'switchify-dev-code-signing-root.cer');
const thumbprintPath = path.join(path.dirname(pfxPath), 'switchify-dev-code-signing.thumbprint');

fs.mkdirSync(path.dirname(pfxPath), { recursive: true });

const escapedPfxPath = escapePowerShellString(pfxPath);
const escapedCerPath = escapePowerShellString(cerPath);
const escapedRootCerPath = escapePowerShellString(rootCerPath);
const escapedThumbprintPath = escapePowerShellString(thumbprintPath);
const escapedPassword = escapePowerShellString(password);

const script = `
$password = ConvertTo-SecureString '${escapedPassword}' -AsPlainText -Force
$root = New-SelfSignedCertificate -Subject 'CN=Switchify PC Dev Code Signing Root' -CertStoreLocation 'Cert:\\CurrentUser\\My' -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -KeyExportPolicy Exportable -KeyUsage CertSign,CRLSign,DigitalSignature -TextExtension @('2.5.29.19={critical}{text}ca=1&pathlength=1','2.5.29.37={text}1.3.6.1.5.5.7.3.3') -NotAfter (Get-Date).AddYears(5)
$cert = New-SelfSignedCertificate -Subject 'CN=Switchify PC Dev Code Signing' -CertStoreLocation 'Cert:\\CurrentUser\\My' -Signer $root -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -KeyExportPolicy Exportable -KeyUsage DigitalSignature -TextExtension @('2.5.29.19={critical}{text}ca=0','2.5.29.37={text}1.3.6.1.5.5.7.3.3') -NotAfter (Get-Date).AddYears(1)
Export-PfxCertificate -Cert $cert -FilePath '${escapedPfxPath}' -Password $password | Out-Null
Export-Certificate -Cert $cert -FilePath '${escapedCerPath}' | Out-Null
Export-Certificate -Cert $root -FilePath '${escapedRootCerPath}' | Out-Null
Set-Content -Path '${escapedThumbprintPath}' -Value $cert.Thumbprint -Encoding ascii
Import-Certificate -FilePath '${escapedRootCerPath}' -CertStoreLocation 'Cert:\\CurrentUser\\Root' | Out-Null
Import-Certificate -FilePath '${escapedCerPath}' -CertStoreLocation 'Cert:\\CurrentUser\\TrustedPublisher' | Out-Null
$machineTrustScript = "Import-Certificate -FilePath '${escapedRootCerPath}' -CertStoreLocation Cert:\\LocalMachine\\Root | Out-Null; Import-Certificate -FilePath '${escapedCerPath}' -CertStoreLocation Cert:\\LocalMachine\\TrustedPublisher | Out-Null"
Start-Process -FilePath powershell.exe -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-Command',$machineTrustScript) -Verb RunAs -Wait
Write-Host 'Created dev code-signing certificate.'
Write-Host 'PFX:' '${escapedPfxPath}'
Write-Host 'Leaf CER:' '${escapedCerPath}'
Write-Host 'Root CER:' '${escapedRootCerPath}'
Write-Host 'Leaf thumbprint:' $cert.Thumbprint
`.trim();

runTool('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', script]);

function escapePowerShellString(value) {
  return value.replace(/'/g, "''");
}
