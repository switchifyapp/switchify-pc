const { spawn } = require('node:child_process');
const { existsSync } = require('node:fs');
const { join, resolve } = require('node:path');

const projectRoot = resolve(__dirname, '..');
const helperPath = join(
  projectRoot,
  'build',
  'native',
  'cursor-overlay-helper',
  'win-x64',
  'SwitchifyCursorOverlay.exe'
);

const timeoutMs = 5_000;
let stdoutBuffer = '';
let stderrBuffer = '';
let settled = false;
let sawReady = false;
let shutdownSent = false;
let timeout = null;
let helper = null;

if (!existsSync(helperPath)) {
  fail(`Cursor overlay helper was not found: ${helperPath}`);
}

helper = spawn(helperPath, [], {
  stdio: ['pipe', 'pipe', 'pipe'],
  windowsHide: true
});

timeout = setTimeout(() => {
  fail(`Cursor overlay helper smoke test timed out.${stderrBuffer ? ` stderr: ${stderrBuffer.trim()}` : ''}`);
}, timeoutMs);

helper.once('error', (error) => {
  fail(`Cursor overlay helper failed to start: ${error.message}`);
});

helper.once('exit', (code, signal) => {
  if (!settled && !shutdownSent) {
    fail(`Cursor overlay helper exited before shutdown: ${signal ?? code ?? 'unknown'}`);
  }
});

helper.stdout.on('data', (chunk) => {
  stdoutBuffer += String(chunk);
  readStdoutLines();
});

helper.stderr.on('data', (chunk) => {
  stderrBuffer += String(chunk);
});

function readStdoutLines() {
  while (stdoutBuffer.includes('\n')) {
    const newlineIndex = stdoutBuffer.indexOf('\n');
    const line = stdoutBuffer.slice(0, newlineIndex).trim();
    stdoutBuffer = stdoutBuffer.slice(newlineIndex + 1);
    if (line) {
      handleMessage(line);
    }
  }
}

function handleMessage(line) {
  let message;
  try {
    message = JSON.parse(line);
  } catch (error) {
    fail(`Cursor overlay helper returned malformed output: ${line}`);
    return;
  }

  if (message.type === 'error') {
    fail(`Cursor overlay helper reported ${message.code ?? 'error'}: ${message.message ?? 'unknown error'}`);
    return;
  }

  if (message.type === 'ready' && !sawReady) {
    sawReady = true;
    smokeEvents();
  }
}

function smokeEvents() {
  for (const event of ['move', 'click', 'drag']) {
    writeCommand({
      type: 'show',
      event,
      x: 100,
      y: 100,
      size: 96,
      durationMs: 50,
      crosshairs: false,
      persistent: false,
      color: {
        red: 211,
        green: 47,
        blue: 47
      }
    });
  }

  setTimeout(() => {
    shutdownSent = true;
    writeCommand({ type: 'shutdown' });
    helper.stdin.end();
    clearTimeout(timeout);
    settled = true;
    console.log('Cursor overlay helper smoke test passed.');
    setTimeout(() => {
      if (helper && !helper.killed) {
        helper.kill();
      }
    }, 1_000).unref();
  }, 100);
}

function writeCommand(command) {
  helper.stdin.write(`${JSON.stringify(command)}\n`);
}

function fail(message) {
  if (settled) return;
  settled = true;
  if (timeout) {
    clearTimeout(timeout);
  }
  if (helper && !helper.killed) {
    helper.kill();
  }
  console.error(message);
  process.exitCode = 1;
}
