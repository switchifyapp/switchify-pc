const fs = require('node:fs');
const path = require('node:path');
const ResEdit = require('resedit');
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

  applyWindowsIcon(executablePath);
  embedUiAccessManifest(executablePath);
  signWindowsExecutable(executablePath);
};

function applyWindowsIcon(executablePath) {
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

  resources.outputResource(executable);
  fs.writeFileSync(executablePath, Buffer.from(executable.generate()));
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

function createSigningArgs(filePath, { requireSigning }) {
  if (!isWindows()) return null;

  if (process.env.SWITCHIFY_SIGNING_MODE === 'azure') {
    return createAzureSigningArgs(filePath, requireSigning);
  }

  const devArgs = createDevPfxSigningArgs(filePath);
  if (devArgs) return devArgs;

  const azureArgs = createAzureSigningArgs(filePath, false);
  if (azureArgs) return azureArgs;

  if (process.env.SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE === '1' && !requireSigning) return null;
  if (process.env.SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE === '1') {
    console.warn('WARNING: Producing an unsigned uiAccess package because SWITCHIFY_ALLOW_UNSIGNED_UIACCESS_PACKAGE=1.');
    return null;
  }

  throw new Error('Signing is required for uiAccess packaging. Set SWITCHIFY_DEV_CERT_PASSWORD with a dev PFX, run npm run signing:create-dev-cert, or configure Azure Artifact Signing.');
}

function createDevPfxSigningArgs(filePath) {
  const password = process.env.SWITCHIFY_DEV_CERT_PASSWORD;
  const pfxPath = process.env.SWITCHIFY_DEV_CERT_PFX || resolveProjectPath('.certs', 'switchify-dev-code-signing.pfx');

  if (!password || !fs.existsSync(pfxPath)) return null;

  const args = ['sign', '/fd', 'SHA256', '/f', pfxPath, '/p', password];
  if (process.env.SWITCHIFY_SIGN_SKIP_TIMESTAMP !== '1') {
    args.push('/tr', 'http://timestamp.digicert.com', '/td', 'SHA256');
  }
  args.push(filePath);
  return args;
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
