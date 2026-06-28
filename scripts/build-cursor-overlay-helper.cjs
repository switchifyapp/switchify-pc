const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { resolveProjectPath } = require('./win-signing-tools.cjs');

const helpers = [
  {
    name: 'cursor overlay helper',
    projectPath: resolveProjectPath('native', 'cursor-overlay-helper', 'CursorOverlayHelper.csproj'),
    outputDir: resolveProjectPath('build', 'native', 'cursor-overlay-helper', 'win-x64'),
    outputExeName: 'SwitchifyCursorOverlay.exe'
  },
  {
    name: 'Bluetooth transport helper',
    projectPath: resolveProjectPath('native', 'bluetooth-transport-helper', 'SwitchifyBluetoothTransport.csproj'),
    outputDir: resolveProjectPath('build', 'native', 'bluetooth-transport-helper', 'win-x64'),
    outputExeName: 'SwitchifyBluetoothTransport.exe'
  },
  {
    name: 'text input helper',
    projectPath: resolveProjectPath('native', 'text-input-helper', 'TextInputHelper.csproj'),
    outputDir: resolveProjectPath('build', 'native', 'text-input-helper', 'win-x64'),
    outputExeName: 'SwitchifyTextInput.exe'
  }
];

for (const helper of helpers) {
  fs.mkdirSync(helper.outputDir, { recursive: true });

  const result = spawnSync(
    'dotnet',
    [
      'publish',
      helper.projectPath,
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
      helper.outputDir
    ],
    {
      cwd: resolveProjectPath(),
      stdio: 'inherit',
      shell: process.platform === 'win32'
    }
  );

  if (result.status !== 0) {
    throw new Error(`dotnet publish failed for ${helper.name} with exit code ${result.status ?? 'unknown'}.`);
  }

  const outputExe = path.join(helper.outputDir, helper.outputExeName);
  if (!fs.existsSync(outputExe)) {
    throw new Error(`Expected ${helper.name} output was not created: ${outputExe}`);
  }

  console.log(`Built ${helper.name}: ${outputExe}`);
}
