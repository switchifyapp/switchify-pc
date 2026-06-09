const fs = require('node:fs');
const path = require('node:path');
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
  }

  return artifacts;
};
