const fs = require('node:fs');
const path = require('node:path');
const { resolveProjectPath } = require('./win-signing-tools.cjs');

const packageJson = require('../package.json');

const distDir = resolveProjectPath('dist-dotnet');
const stageDir = path.join(distDir, 'win-unpacked');
const appExe = path.join(stageDir, 'Switchify PC.exe');
const builderDebug = path.join(distDir, 'builder-debug.yml');
const latestYml = path.join(distDir, 'latest.yml');
const expectedInstaller = path.join(distDir, `Switchify-PC-Setup-${packageJson.version}-x64.exe`);
const installerScript = resolveProjectPath('installer', 'SwitchifyPc.DotNet.nsi');

assertExists(appExe, 'staged C# app executable');
assertExists(builderDebug, 'C# builder debug metadata');
assertInstallerScript();
assertNoElectronRuntime();

if (fs.existsSync(expectedInstaller) || fs.existsSync(latestYml)) {
  assertExists(expectedInstaller, 'C# NSIS installer');
  assertExists(latestYml, 'C# updater metadata');
  const latest = fs.readFileSync(latestYml, 'utf8');
  assertIncludes(latest, `version: ${packageJson.version}`, 'latest.yml version');
  assertIncludes(latest, `path: ${path.basename(expectedInstaller)}`, 'latest.yml installer path');
  assertRegex(latest, /^isAdminRightsRequired:\s*true\s*$/m, 'top-level admin metadata');
  assertRegex(latest, /^    isAdminRightsRequired:\s*true\s*$/m, 'file-entry admin metadata');
}

console.log('C# package verification passed.');

function assertExists(filePath, label) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`${label} was not found: ${filePath}`);
  }
}

function assertNoElectronRuntime() {
  const electronMarkers = [
    path.join(stageDir, 'resources', 'app.asar'),
    path.join(stageDir, 'chrome_100_percent.pak'),
    path.join(stageDir, 'ffmpeg.dll'),
    path.join(stageDir, 'resources', 'electron.asar')
  ];

  for (const marker of electronMarkers) {
    if (fs.existsSync(marker)) {
      throw new Error(`C# package contains Electron runtime marker: ${marker}`);
    }
  }
}

function assertInstallerScript() {
  assertExists(installerScript, 'C# NSIS installer script');
  const content = fs.readFileSync(installerScript, 'utf8');
  assertIncludes(content, 'RequestExecutionLevel admin', 'installer elevation requirement');
  assertIncludes(content, 'InstallDir "$PROGRAMFILES64\\Switchify PC"', 'installer Program Files target');
  assertIncludes(content, '!define MUI_FINISHPAGE_RUN "$INSTDIR\\Switchify PC.exe"', 'installer run-after-finish behavior');
  assertIncludes(content, 'Call PromptForRunningApp', 'installer running-app prompt');
  assertIncludes(content, 'Call un.PromptForRunningApp', 'uninstaller running-app prompt');
  assertIncludes(content, 'find /I "Switchify PC.exe"', 'installer process detection');
  assertIncludes(
    content,
    'Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Switchify PC',
    'installer uninstall registry key'
  );
}

function assertIncludes(content, expected, label) {
  if (!content.includes(expected)) {
    throw new Error(`${label} is missing ${expected}.`);
  }
}

function assertRegex(content, regex, label) {
  if (!regex.test(content)) {
    throw new Error(`${label} is missing.`);
  }
}
