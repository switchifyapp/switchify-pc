const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
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
const helperPath = resolveProjectPath('dist', 'win-unpacked', 'resources', 'native', 'SwitchifyCursorOverlay.exe');
if (!fs.existsSync(helperPath)) {
  throw new Error(`Native cursor overlay helper not found: ${helperPath}`);
}
const bluetoothHelperPath = resolveProjectPath(
  'dist',
  'win-unpacked',
  'resources',
  'native',
  'SwitchifyBluetoothTransport.exe'
);
if (!fs.existsSync(bluetoothHelperPath)) {
  throw new Error(`Native Bluetooth transport helper not found: ${bluetoothHelperPath}`);
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
  const helperSignatureResult = runTool(signtoolExe, ['verify', '/pa', '/v', helperPath], { stdio: 'pipe' });
  const helperSignatureOutput = `${helperSignatureResult.stdout || ''}${helperSignatureResult.stderr || ''}`;
  const bluetoothHelperSignatureResult = runTool(signtoolExe, ['verify', '/pa', '/v', bluetoothHelperPath], {
    stdio: 'pipe'
  });
  const bluetoothHelperSignatureOutput = `${bluetoothHelperSignatureResult.stdout || ''}${bluetoothHelperSignatureResult.stderr || ''}`;

  console.log(`manifest embedded: yes`);
  console.log(`uiAccess=true: ${hasUiAccess ? 'yes' : 'no'}`);
  console.log(`highestAvailable: ${hasHighestAvailable ? 'yes' : 'no'}`);
  console.log(`signature status: ${signatureOutput.includes('Successfully verified') ? 'valid' : 'check output above'}`);
  console.log('cursor overlay helper: present');
  console.log(`cursor overlay helper signature: ${helperSignatureOutput.includes('Successfully verified') ? 'valid' : 'check output above'}`);
  console.log('Bluetooth transport helper: present');
  console.log(`Bluetooth transport helper signature: ${bluetoothHelperSignatureOutput.includes('Successfully verified') ? 'valid' : 'check output above'}`);
  console.log('secure install location required: install per-machine under Program Files for uiAccess to take effect.');

  if (!hasUiAccess || !hasHighestAvailable) {
    throw new Error('Packaged executable manifest does not contain the required uiAccess settings.');
  }

  if (process.env.SWITCHIFY_VERIFY_INSTALLED_LAUNCH === '1') {
    verifyInstalledLaunch();
  }
} finally {
  fs.rmSync(manifestOutPath, { force: true });
}

function verifyInstalledLaunch() {
  const installedPath = path.join(process.env.ProgramFiles || 'C:\\Program Files', 'Switchify PC', 'Switchify PC.exe');
  if (!fs.existsSync(installedPath)) {
    throw new Error(`Installed executable not found: ${installedPath}`);
  }

  const launchScript = `
$path = '${escapePowerShellString(installedPath)}'
$ErrorActionPreference = 'Stop'
$process = Start-Process -FilePath $path -PassThru
Start-Sleep -Seconds 5
$launched = Get-CimInstance Win32_Process | Where-Object { $_.ProcessId -eq $process.Id -and $_.ExecutablePath -eq $path }
if ($null -eq $launched) {
  Write-Host 'installed launch: failed'
  exit 1
}
Write-Host ('installed launch: running (pid {0})' -f $process.Id)
`.trim();

  const result = spawnSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', launchScript], {
    encoding: 'utf8',
    stdio: 'pipe'
  });
  const output = `${result.stdout || ''}${result.stderr || ''}`.trim();
  if (output) {
    console.log(output);
  }
  if (result.status !== 0) {
    throw new Error('Installed uiAccess executable did not stay running.');
  }
}

function escapePowerShellString(value) {
  return value.replace(/'/g, "''");
}
