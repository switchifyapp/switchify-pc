const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

function isWindows() {
  return process.platform === 'win32';
}

function resolveProjectPath(...segments) {
  return path.resolve(__dirname, '..', ...segments);
}

function findWindowsSdkTool(toolName) {
  const envOverride = toolName.toLowerCase() === 'mt.exe' ? process.env.SWITCHIFY_MT_EXE : process.env.SWITCHIFY_SIGNTOOL_EXE;
  if (envOverride) {
    if (!fs.existsSync(envOverride)) {
      throw new Error(`${path.basename(envOverride)} was configured but does not exist: ${envOverride}`);
    }
    return envOverride;
  }

  const sdkBinRoot = 'C:\\Program Files (x86)\\Windows Kits\\10\\bin';
  const sdkTool = findNewestSdkTool(sdkBinRoot, toolName);
  if (sdkTool) return sdkTool;

  const whereResult = spawnSync('where.exe', [toolName], { encoding: 'utf8' });
  if (whereResult.status === 0) {
    const [firstMatch] = whereResult.stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    if (firstMatch) return firstMatch;
  }

  throw new Error(`Unable to find ${toolName}. Install the Windows SDK or set ${toolName.toLowerCase() === 'mt.exe' ? 'SWITCHIFY_MT_EXE' : 'SWITCHIFY_SIGNTOOL_EXE'}.`);
}

function runTool(command, args, options = {}) {
  const displayCommand = [command, ...redactSensitiveArgs(args)].join(' ');
  const result = spawnSync(command, args, {
    stdio: options.stdio ?? 'inherit',
    encoding: 'utf8',
    ...options
  });

  if (result.status !== 0) {
    throw new Error(`${displayCommand} failed with exit code ${result.status ?? 'unknown'}.`);
  }

  return result;
}

function findNewestSdkTool(sdkBinRoot, toolName) {
  if (!fs.existsSync(sdkBinRoot)) return null;

  const versions = fs.readdirSync(sdkBinRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort(compareVersionDescending);

  for (const version of versions) {
    const candidate = path.join(sdkBinRoot, version, 'x64', toolName);
    if (fs.existsSync(candidate)) return candidate;
  }

  return null;
}

function compareVersionDescending(left, right) {
  const leftParts = left.split('.').map((part) => Number.parseInt(part, 10));
  const rightParts = right.split('.').map((part) => Number.parseInt(part, 10));
  const length = Math.max(leftParts.length, rightParts.length);

  for (let index = 0; index < length; index += 1) {
    const delta = (rightParts[index] || 0) - (leftParts[index] || 0);
    if (delta !== 0) return delta;
  }

  return right.localeCompare(left);
}

function redactSensitiveArgs(args) {
  return args.map((arg, index) => {
    const previous = args[index - 1]?.toLowerCase();
    if (previous === '/p' || previous === '-p') return '<redacted>';
    return String(arg);
  });
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

  throw new Error(
    'Signing is required for uiAccess packaging. Set SWITCHIFY_DEV_CERT_PASSWORD with a dev PFX, run the dev certificate script, or configure Azure Artifact Signing.'
  );
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

module.exports = {
  createSigningArgs,
  findWindowsSdkTool,
  isWindows,
  resolveProjectPath,
  runTool
};
