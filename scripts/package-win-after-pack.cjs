const fs = require('node:fs');
const path = require('node:path');
const ResEdit = require('resedit');
const packageJson = require('../package.json');
const {
  findWindowsSdkTool,
  isWindows,
  resolveProjectPath,
  runTool
} = require('./win-signing-tools.cjs');

module.exports = async function packageWindowsAfterPack(context) {
  if (context.electronPlatformName !== 'win32') {
    return;
  }

  const executableName = `${context.packager.appInfo.productFilename}.exe`;
  const executablePath = path.join(context.appOutDir, executableName);

  applyWindowsExecutableResources(executablePath);
  embedUiAccessManifest(executablePath);
  signWindowsExecutable(executablePath);
  signNativeHelpers(context.appOutDir);
};

function applyWindowsExecutableResources(executablePath) {
  const iconPath = resolveProjectPath('build', 'icon.ico');
  const executable = ResEdit.NtExecutable.from(fs.readFileSync(executablePath), { ignoreCert: true });
  const resources = ResEdit.NtExecutableResource.from(executable);
  const iconFile = ResEdit.Data.IconFile.from(fs.readFileSync(iconPath));

  ResEdit.Resource.IconGroupEntry.replaceIconsForResource(
    resources.entries,
    1,
    1033,
    iconFile.icons.map((item) => item.data)
  );

  applyWindowsVersionInfo(resources);
  resources.outputResource(executable);
  fs.writeFileSync(executablePath, Buffer.from(executable.generate()));
}

function applyWindowsVersionInfo(resources) {
  const versionInfo = ResEdit.Resource.VersionInfo.fromEntries(resources.entries)[0];
  if (!versionInfo) {
    throw new Error('Unable to find executable version info resource.');
  }

  const [major, minor, patch, build] = parseWindowsVersion(packageJson.version);
  versionInfo.setFileVersion(major, minor, patch, build, 1033);
  versionInfo.setProductVersion(major, minor, patch, build, 1033);
  versionInfo.setStringValues(
    { lang: 1033, codepage: 1200 },
    {
      CompanyName: 'Switchify',
      FileDescription: 'Switchify PC',
      FileVersion: packageJson.version,
      InternalName: 'Switchify PC',
      LegalCopyright: 'Copyright (C) 2026 Owen McGirr',
      OriginalFilename: 'Switchify PC.exe',
      ProductName: 'Switchify PC',
      ProductVersion: packageJson.version
    }
  );
  versionInfo.outputToResourceEntries(resources.entries);
}

function parseWindowsVersion(version) {
  const numericParts = String(version)
    .split(/[.-]/)
    .slice(0, 4)
    .map((part) => Number.parseInt(part, 10))
    .map((part) => (Number.isFinite(part) && part >= 0 ? part : 0));

  while (numericParts.length < 4) {
    numericParts.push(0);
  }

  return numericParts;
}

function embedUiAccessManifest(executablePath) {
  const mtExe = findWindowsSdkTool('mt.exe');
  const manifestPath = resolveProjectPath('build', 'win-uiaccess.manifest');
  runTool(mtExe, ['-nologo', '-manifest', manifestPath, `-outputresource:${executablePath};#1`]);
}

function signWindowsExecutable(filePath) {
  const signingArgs = createSigningArgs(filePath, { requireSigning: true });
  if (!signingArgs) return;

  const signtoolExe = findWindowsSdkTool('signtool.exe');
  runTool(signtoolExe, signingArgs);
}

function signNativeHelpers(appOutDir) {
  for (const helperName of ['SwitchifyCursorOverlay.exe', 'SwitchifyBluetoothTransport.exe']) {
    const helperPath = path.join(appOutDir, 'resources', 'native', helperName);
    if (!fs.existsSync(helperPath)) {
      throw new Error(`Native helper is missing from packaged resources: ${helperPath}`);
    }

    signWindowsExecutable(helperPath);
  }
}

function createSigningArgs(filePath, { requireSigning }) {
  if (!isWindows()) return null;

  if (process.env.SWITCHIFY_SIGNING_MODE === 'certum-simplysign') {
    return createCertumSimplySignArgs(filePath, requireSigning);
  }

  if (process.env.SWITCHIFY_SIGNING_MODE === 'azure') {
    return createAzureSigningArgs(filePath, requireSigning);
  }

  const devArgs = createDevPfxSigningArgs(filePath);
  if (devArgs) return devArgs;

  const devStoreArgs = createDevStoreSigningArgs(filePath);
  if (devStoreArgs) return devStoreArgs;

  const azureArgs = createAzureSigningArgs(filePath, false);
  if (azureArgs) return azureArgs;

  if (process.env.SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE === '1' && !requireSigning) return null;
  if (process.env.SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE === '1') {
    console.warn('WARNING: Producing an unsigned uiAccess package because SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE=1.');
    return null;
  }

  throw new Error('Signing is required for uiAccess packaging. Set SWITCHIFY_DEV_CERT_PASSWORD with a dev PFX, run npm run signing:create-dev-cert, or configure Azure Artifact Signing.');
}

function createCertumSimplySignArgs(filePath, requireSigning) {
  const thumbprint = normalizeThumbprint(process.env.SWITCHIFY_CERTUM_CERT_THUMBPRINT || '');

  if (!thumbprint) {
    if (requireSigning && process.env.SWITCHIFY_SIGNING_MODE === 'certum-simplysign') {
      throw new Error('Certum SimplySign signing requires SWITCHIFY_CERTUM_CERT_THUMBPRINT.');
    }
    return null;
  }

  return [
    'sign',
    '/v',
    '/fd',
    'SHA256',
    '/sha1',
    thumbprint,
    '/tr',
    process.env.SWITCHIFY_CERTUM_TIMESTAMP_URL || 'http://time.certum.pl',
    '/td',
    'SHA256',
    filePath
  ];
}

function createDevPfxSigningArgs(filePath) {
  const password = process.env.SWITCHIFY_DEV_CERT_PASSWORD;
  const pfxPath = process.env.SWITCHIFY_DEV_CERT_PFX || resolveProjectPath('.certs', 'switchify-dev-code-signing.pfx');
  const thumbprint = resolveDevCertificateThumbprint(pfxPath);

  if (!password || !fs.existsSync(pfxPath)) return null;

  const args = ['sign', '/fd', 'SHA256', '/f', pfxPath, '/p', password];
  if (thumbprint) {
    args.push('/sha1', thumbprint);
  }
  if (process.env.SWITCHIFY_SIGN_SKIP_TIMESTAMP !== '1') {
    args.push('/tr', 'http://timestamp.digicert.com', '/td', 'SHA256');
  }
  args.push(filePath);
  return args;
}

function createDevStoreSigningArgs(filePath) {
  const thumbprint = resolveDevCertificateThumbprint(
    process.env.SWITCHIFY_DEV_CERT_PFX || resolveProjectPath('.certs', 'switchify-dev-code-signing.pfx')
  );

  if (!thumbprint) return null;

  const args = ['sign', '/fd', 'SHA256', '/sha1', thumbprint];
  if (process.env.SWITCHIFY_SIGN_SKIP_TIMESTAMP !== '1') {
    args.push('/tr', 'http://timestamp.digicert.com', '/td', 'SHA256');
  }
  args.push(filePath);
  return args;
}

function resolveDevCertificateThumbprint(pfxPath) {
  if (process.env.SWITCHIFY_DEV_CERT_THUMBPRINT) {
    return normalizeThumbprint(process.env.SWITCHIFY_DEV_CERT_THUMBPRINT);
  }

  const thumbprintPath = path.join(path.dirname(pfxPath), 'switchify-dev-code-signing.thumbprint');
  if (!fs.existsSync(thumbprintPath)) return null;

  return normalizeThumbprint(fs.readFileSync(thumbprintPath, 'utf8'));
}

function normalizeThumbprint(value) {
  const thumbprint = value.replace(/[^a-fA-F0-9]/g, '').toUpperCase();
  return thumbprint.length > 0 ? thumbprint : null;
}

function createAzureSigningArgs(filePath, requireSigning) {
  const dlibPath = process.env.SWITCHIFY_AZURE_SIGNING_DLIB;
  const metadataPath = process.env.SWITCHIFY_AZURE_SIGNING_METADATA;

  if (!dlibPath || !metadataPath) {
    if (requireSigning && process.env.SWITCHIFY_SIGNING_MODE === 'azure') {
      throw new Error('Azure Artifact Signing requires SWITCHIFY_AZURE_SIGNING_DLIB and SWITCHIFY_AZURE_SIGNING_METADATA.');
    }
    return null;
  }

  return [
    'sign',
    '/v',
    '/debug',
    '/fd',
    'SHA256',
    '/tr',
    'http://timestamp.acs.microsoft.com',
    '/td',
    'SHA256',
    '/dlib',
    dlibPath,
    '/dmdf',
    metadataPath,
    filePath
  ];
}

module.exports.createSigningArgs = createSigningArgs;
