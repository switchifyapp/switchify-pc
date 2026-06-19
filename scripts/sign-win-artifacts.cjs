const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const {
  findWindowsSdkTool,
  isWindows,
  resolveProjectPath,
  runTool
} = require('./win-signing-tools.cjs');
const { createSigningArgs } = require('./package-win-after-pack.cjs');

module.exports = async function signWindowsArtifacts(buildResult) {
  if (!isWindows()) {
    return [];
  }

  const distDir = resolveProjectPath('dist');
  const artifacts = buildResult.artifactPaths && buildResult.artifactPaths.length > 0
    ? buildResult.artifactPaths
    : fs.readdirSync(distDir)
      .filter((entry) => entry.toLowerCase().endsWith('.exe'))
      .map((entry) => path.join(distDir, entry));

  const installerArtifacts = artifacts.filter((artifactPath) => {
    const normalized = path.normalize(artifactPath);
    return normalized.toLowerCase().endsWith('.exe') && !normalized.toLowerCase().includes(`${path.sep}win-unpacked${path.sep}`);
  });

  if (installerArtifacts.length === 0) {
    return artifacts;
  }

  const signtoolExe = findWindowsSdkTool('signtool.exe');
  for (const artifactPath of installerArtifacts) {
    const signingArgs = createSigningArgs(artifactPath, { requireSigning: process.env.SWITCHIFY_ALLOW_UNSIGNED_INSTALLER !== '1' });
    if (!signingArgs) {
      console.warn(`WARNING: Skipping installer signing for ${artifactPath}.`);
      continue;
    }
    runTool(signtoolExe, signingArgs);
    updateLatestYmlForSignedInstaller(artifactPath);
  }

  return artifacts;
};

if (require.main === module) {
  const command = process.argv[2];
  if (command !== '--update-latest-yml') {
    throw new Error('Unsupported command. Use --update-latest-yml.');
  }

  updateLatestYmlForReferencedInstaller(resolveProjectPath('dist', 'latest.yml'));
}

function updateLatestYmlForSignedInstaller(installerPath) {
  const latestPath = path.join(path.dirname(installerPath), 'latest.yml');
  if (!fs.existsSync(latestPath)) {
    return;
  }

  const installerName = path.basename(installerPath);
  const sha512 = createSha512Base64(installerPath);
  const content = fs.readFileSync(latestPath, 'utf8');

  if (!referencesInstaller(content, installerName)) {
    console.warn(`WARNING: Skipping latest.yml update because it does not reference ${installerName}.`);
    return;
  }

  const updated = content.replace(/^(\s*sha512:\s*).+$/gm, `$1${sha512}`);

  if (updated === content) {
    throw new Error('latest.yml did not contain any sha512 entries to update.');
  }

  fs.writeFileSync(latestPath, updated);
  console.log(`Updated latest.yml sha512 for ${installerName}.`);
}

function updateLatestYmlForReferencedInstaller(latestPath) {
  if (!fs.existsSync(latestPath)) {
    return;
  }

  const content = fs.readFileSync(latestPath, 'utf8');
  const installerName = findInstallerNameInLatestYml(content);
  const installerPath = path.join(path.dirname(latestPath), installerName);

  if (!fs.existsSync(installerPath)) {
    throw new Error(`latest.yml references missing installer ${installerName}.`);
  }

  updateLatestYmlForSignedInstaller(installerPath);
}

function createSha512Base64(filePath) {
  return crypto.createHash('sha512').update(fs.readFileSync(filePath)).digest('base64');
}

function findInstallerNameInLatestYml(content) {
  const match = content.match(/^\s*(?:url|path):\s*['"]?([^'"\r\n]+\.exe)['"]?\s*$/im);
  if (!match) {
    throw new Error('latest.yml does not reference a Windows installer.');
  }

  return path.basename(decodeURI(match[1]));
}

function referencesInstaller(content, installerName) {
  const encodedInstallerName = encodeURI(installerName);
  return (
    content.includes(`url: ${installerName}`) ||
    content.includes(`path: ${installerName}`) ||
    content.includes(`url: ${encodedInstallerName}`) ||
    content.includes(`path: ${encodedInstallerName}`)
  );
}

module.exports.updateLatestYmlForSignedInstaller = updateLatestYmlForSignedInstaller;
module.exports.updateLatestYmlForReferencedInstaller = updateLatestYmlForReferencedInstaller;
module.exports.createSha512Base64 = createSha512Base64;
