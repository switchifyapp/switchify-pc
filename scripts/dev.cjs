const { spawn } = require('node:child_process');
const { join } = require('node:path');

const env = Object.fromEntries(
  Object.entries(process.env).filter(
    ([key, value]) => key !== 'ELECTRON_RUN_AS_NODE' && typeof value === 'string'
  )
);

const electronViteBin = join(__dirname, '..', 'node_modules', 'electron-vite', 'bin', 'electron-vite.js');
const child = spawn(process.execPath, [electronViteBin, 'dev'], {
  env,
  stdio: 'inherit',
  shell: false
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 0);
});
