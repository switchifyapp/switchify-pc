const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { resolveProjectPath } = require('./win-signing-tools.cjs');

const projectPath = resolveProjectPath('native', 'cursor-overlay-helper', 'CursorOverlayHelper.csproj');
const outputDir = resolveProjectPath('build', 'native', 'cursor-overlay-helper', 'win-x64');
const outputExe = path.join(outputDir, 'SwitchifyCursorOverlay.exe');

fs.mkdirSync(outputDir, { recursive: true });

const result = spawnSync(
  'dotnet',
  [
    'publish',
    projectPath,
    '-c',
    'Release',
    '-r',
    'win-x64',
    '--self-contained',
    'true',
    '-p:PublishSingleFile=true',
    '-p:PublishTrimmed=false',
    '-p:ReadyToRun=false',
    '-o',
    outputDir
  ],
  {
    cwd: resolveProjectPath(),
    stdio: 'inherit',
    shell: process.platform === 'win32'
  }
);

if (result.status !== 0) {
  throw new Error(`dotnet publish failed with exit code ${result.status ?? 'unknown'}.`);
}

if (!fs.existsSync(outputExe)) {
  throw new Error(`Expected cursor overlay helper output was not created: ${outputExe}`);
}

console.log(`Built cursor overlay helper: ${outputExe}`);
