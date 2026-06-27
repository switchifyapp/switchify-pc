const fs = require('node:fs');
const { spawnSync } = require('node:child_process');
const { resolveProjectPath } = require('./win-signing-tools.cjs');

const helperPath = resolveProjectPath('build', 'native', 'update-launcher-helper', 'win-x64', 'SwitchifyUpdateLauncher.exe');
if (!fs.existsSync(helperPath)) {
  throw new Error(`Update launcher helper was not built: ${helperPath}`);
}

const result = spawnSync(helperPath, ['--self-test-quote'], {
  encoding: 'utf8',
  stdio: 'pipe'
});

if (result.status !== 0) {
  throw new Error(`Update launcher helper self-test failed with exit code ${result.status ?? 'unknown'}: ${result.stderr}`);
}

const output = result.stdout.trim();
if (output !== '--updated --force-run "value with spaces"') {
  throw new Error(`Unexpected update launcher self-test output: ${output}`);
}

console.log('Update launcher helper self-test passed.');
