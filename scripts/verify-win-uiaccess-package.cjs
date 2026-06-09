const fs = require('node:fs');
const path = require('node:path');
const {
  findWindowsSdkTool,
  isWindows,
  resolveProjectPath,
  runTool
} = require('./win-signing-tools.cjs');

if (!isWindows()) {
  throw new Error('uiAccess package verification must run on Windows.');
}

const executablePath = resolveProjectPath('dist', 'win-unpacked', 'Switchify PC.exe');
if (!fs.existsSync(executablePath)) {
  throw new Error(`Packaged executable not found: ${executablePath}`);
}

const mtExe = findWindowsSdkTool('mt.exe');
const signtoolExe = findWindowsSdkTool('signtool.exe');
const manifestOutPath = path.join(path.dirname(executablePath), 'Switchify PC.exe.manifest');

try {
  runTool(mtExe, ['-nologo', `-inputresource:${executablePath};#1`, `-out:${manifestOutPath}`]);
  const manifest = fs.readFileSync(manifestOutPath, 'utf8');
  const hasUiAccess = /uiAccess\s*=\s*"true"/.test(manifest);
  const hasHighestAvailable = /level\s*=\s*"highestAvailable"/.test(manifest);

  const signatureResult = runTool(signtoolExe, ['verify', '/pa', '/v', executablePath], { stdio: 'pipe' });
  const signatureOutput = `${signatureResult.stdout || ''}${signatureResult.stderr || ''}`;

  console.log(`manifest embedded: yes`);
  console.log(`uiAccess=true: ${hasUiAccess ? 'yes' : 'no'}`);
  console.log(`highestAvailable: ${hasHighestAvailable ? 'yes' : 'no'}`);
  console.log(`signature status: ${signatureOutput.includes('Successfully verified') ? 'valid' : 'check output above'}`);
  console.log('secure install location required: install per-machine under Program Files for uiAccess to take effect.');

  if (!hasUiAccess || !hasHighestAvailable) {
    throw new Error('Packaged executable manifest does not contain the required uiAccess settings.');
  }
} finally {
  fs.rmSync(manifestOutPath, { force: true });
}
