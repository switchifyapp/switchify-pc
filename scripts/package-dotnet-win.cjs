const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const {
  createSigningArgs,
  findWindowsSdkTool,
  isWindows,
  resolveProjectPath,
  runTool
} = require('./win-signing-tools.cjs');

const stageOnly = process.argv.includes('--stage-only');
const skipSign = process.argv.includes('--skip-sign');
const appProjectPath = resolveProjectPath('src-dotnet', 'SwitchifyPc.App', 'SwitchifyPc.App.csproj');
const version = readDotnetAppVersion(appProjectPath);
const publishDir = resolveProjectPath(
  'src-dotnet',
  'SwitchifyPc.App',
  'bin',
  'Release',
  'net8.0-windows10.0.19041.0',
  'win-x64',
  'publish'
);
const distDir = resolveProjectPath('dist-dotnet');
const stageDir = path.join(distDir, 'win-unpacked');
const installerName = `Switchify-PC-Setup-${version}-x64.exe`;
const installerPath = path.join(distDir, installerName);

runDotnetPublish();
resetPackageArtifacts();
resetDirectory(stageDir);
copyDirectory(publishDir, stageDir);
verifyStagedApp();
writeBuilderDebug();

const appExe = path.join(stageDir, 'Switchify PC.exe');
if (isWindows() && !skipSign) {
  embedUiAccessManifest(appExe);
  signFile(appExe, { requireSigning: true });
}

if (!stageOnly) {
  buildInstaller();
  if (isWindows() && !skipSign) {
    signFile(installerPath, { requireSigning: true });
  }
  writeLatestYml();
}

console.log(stageOnly ? `Staged C# app in ${stageDir}` : `Packaged C# installer at ${installerPath}`);

function runDotnetPublish() {
  const result = spawnSync(
    'dotnet',
    [
      'publish',
      appProjectPath,
      '-c',
      'Release',
      '-r',
      'win-x64',
      '--self-contained',
      'true'
    ],
    { stdio: 'inherit' }
  );
  if (result.status !== 0) {
    throw new Error(`dotnet publish failed with exit code ${result.status ?? 'unknown'}.`);
  }
}

function resetDirectory(directory) {
  fs.rmSync(directory, { recursive: true, force: true });
  fs.mkdirSync(directory, { recursive: true });
}

function resetPackageArtifacts() {
  fs.mkdirSync(distDir, { recursive: true });
  fs.rmSync(path.join(distDir, 'latest.yml'), { force: true });
  for (const entry of fs.readdirSync(distDir)) {
    if (/^Switchify-PC-Setup-.+-x64\.exe$/i.test(entry)) {
      fs.rmSync(path.join(distDir, entry), { force: true });
    }
  }
}

function copyDirectory(from, to) {
  if (!fs.existsSync(from)) {
    throw new Error(`Publish directory was not found: ${from}`);
  }

  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    const source = path.join(from, entry.name);
    const target = path.join(to, entry.name);
    if (entry.isDirectory()) {
      fs.mkdirSync(target, { recursive: true });
      copyDirectory(source, target);
    } else {
      fs.copyFileSync(source, target);
    }
  }
}

function verifyStagedApp() {
  const executable = path.join(stageDir, 'Switchify PC.exe');
  if (!fs.existsSync(executable)) {
    throw new Error(`Staged C# app executable is missing: ${executable}`);
  }

  const electronMarkers = ['resources', 'chrome_100_percent.pak', 'ffmpeg.dll'];
  for (const marker of electronMarkers) {
    if (fs.existsSync(path.join(stageDir, marker))) {
      throw new Error(`Staged C# app unexpectedly contains Electron marker: ${marker}`);
    }
  }
}

function embedUiAccessManifest(executablePath) {
  const mtExe = findWindowsSdkTool('mt.exe');
  const manifestPath = resolveProjectPath('build', 'win-uiaccess.manifest');
  runTool(mtExe, ['-nologo', '-manifest', manifestPath, `-outputresource:${executablePath};#1`]);
}

function signFile(filePath, { requireSigning }) {
  const signingArgs = createSigningArgs(filePath, { requireSigning });
  if (!signingArgs) return;

  const signtoolExe = findWindowsSdkTool('signtool.exe');
  runTool(signtoolExe, signingArgs);
}

function buildInstaller() {
  const makensis = findMakensis();
  fs.rmSync(installerPath, { force: true });
  runTool(makensis, [
    `/DVERSION=${version}`,
    `/DSOURCE_DIR=${stageDir}`,
    `/DOUTPUT_EXE=${installerPath}`,
    resolveProjectPath('installer', 'SwitchifyPc.DotNet.nsi')
  ]);
  if (!fs.existsSync(installerPath)) {
    throw new Error(`NSIS did not create expected installer: ${installerPath}`);
  }
}

function findMakensis() {
  const override = process.env.SWITCHIFY_MAKENSIS_EXE;
  if (override) {
    if (!fs.existsSync(override)) {
      throw new Error(`SWITCHIFY_MAKENSIS_EXE does not exist: ${override}`);
    }
    return override;
  }

  const candidates = [
    'C:\\Program Files (x86)\\NSIS\\makensis.exe',
    'C:\\Program Files\\NSIS\\makensis.exe'
  ];
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }

  const result = spawnSync('where.exe', ['makensis.exe'], { encoding: 'utf8' });
  if (result.status === 0) {
    const [first] = result.stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    if (first) return first;
  }

  throw new Error('Unable to find makensis.exe. Install NSIS or set SWITCHIFY_MAKENSIS_EXE.');
}

function writeLatestYml() {
  const sha512 = createSha512Base64(installerPath);
  const size = fs.statSync(installerPath).size;
  const releaseDate = new Date().toISOString();
  const latest = [
    `version: ${version}`,
    'files:',
    `  - url: ${installerName}`,
    `    sha512: ${sha512}`,
    `    size: ${size}`,
    '    isAdminRightsRequired: true',
    `path: ${installerName}`,
    `sha512: ${sha512}`,
    'isAdminRightsRequired: true',
    `releaseDate: '${releaseDate}'`,
    ''
  ].join('\n');

  fs.writeFileSync(path.join(distDir, 'latest.yml'), latest);
}

function writeBuilderDebug() {
  const content = [
    `version: ${version}`,
    'packager: dotnet-wpf-nsis',
    `stageDir: ${toYamlPath(stageDir)}`,
    `publishDir: ${toYamlPath(publishDir)}`,
    ''
  ].join('\n');
  fs.writeFileSync(path.join(distDir, 'builder-debug.yml'), content);
}

function createSha512Base64(filePath) {
  return crypto.createHash('sha512').update(fs.readFileSync(filePath)).digest('base64');
}

function toYamlPath(value) {
  return JSON.stringify(value.replace(/\\/g, '/'));
}

function readDotnetAppVersion(projectPath) {
  const content = fs.readFileSync(projectPath, 'utf8');
  const match = content.match(/<Version>([^<]+)<\/Version>/);
  if (!match || !match[1].trim()) {
    throw new Error(`Could not read C# app <Version> from ${projectPath}.`);
  }

  return match[1].trim();
}
