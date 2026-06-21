const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { resolveProjectPath } = require('./win-signing-tools.cjs');

const helperPath = resolveProjectPath('build', 'native', 'text-input-helper', 'win-x64', 'SwitchifyTextInput.exe');
const TIMEOUT_MS = 5_000;

if (!fs.existsSync(helperPath)) {
  console.error(`Text input helper was not found: ${helperPath}`);
  process.exit(1);
}

const helper = spawn(helperPath, [], {
  stdio: ['pipe', 'pipe', 'pipe'],
  windowsHide: true
});

let stdoutBuffer = '';
let stderr = '';
const pendingMessages = [];
let finished = false;
let expectingExit = false;

const timeout = setTimeout(() => {
  fail(`Timed out after ${TIMEOUT_MS}ms waiting for text input helper.`);
}, TIMEOUT_MS);

helper.stdout.on('data', (chunk) => {
  stdoutBuffer += String(chunk);
  let newlineIndex;
  while ((newlineIndex = stdoutBuffer.search(/\r?\n/)) >= 0) {
    const line = stdoutBuffer.slice(0, newlineIndex).trim();
    stdoutBuffer = stdoutBuffer.slice(newlineIndex + (stdoutBuffer[newlineIndex] === '\r' ? 2 : 1));
    if (!line) continue;
    try {
      pendingMessages.push(JSON.parse(line));
    } catch (error) {
      fail(`Text input helper returned malformed JSON: ${line}`);
    }
  }
});

helper.stderr.on('data', (chunk) => {
  stderr += String(chunk);
});

helper.once('error', (error) => {
  fail(`Text input helper failed to start: ${error.message}`);
});

helper.once('exit', (code, signal) => {
  if (!finished && !expectingExit) {
    fail(`Text input helper exited early: ${signal ?? code ?? 'unknown'}`);
  }
});

main().catch((error) => fail(error instanceof Error ? error.message : String(error)));

async function main() {
  const ready = await waitForMessage((message) => message.type === 'ready');
  if (!ready) throw new Error('Text input helper did not report ready.');

  send({ type: 'typeText', id: 'smoke-empty', text: '' });
  const emptyResult = await waitForMessage((message) => message.id === 'smoke-empty');
  if (!emptyResult?.ok || emptyResult.sentEvents !== 0) {
    throw new Error(`Empty text smoke command failed: ${JSON.stringify(emptyResult)}`);
  }

  send({ type: 'unsupported', id: 'smoke-invalid' });
  const invalidResult = await waitForMessage((message) => message.id === 'smoke-invalid');
  if (invalidResult?.type !== 'error' || invalidResult.code !== 'invalid_command') {
    throw new Error(`Invalid command did not return a structured error: ${JSON.stringify(invalidResult)}`);
  }

  send({ type: 'shutdown' });
  expectingExit = true;
  await waitForExit();
  finish();
}

function send(command) {
  helper.stdin.write(`${JSON.stringify(command)}\n`);
}

async function waitForMessage(predicate) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < TIMEOUT_MS) {
    const index = pendingMessages.findIndex(predicate);
    if (index >= 0) {
      const [message] = pendingMessages.splice(index, 1);
      return message;
    }
    await delay(25);
  }
  return null;
}

async function waitForExit() {
  if (helper.exitCode !== null || helper.signalCode !== null) return;
  await new Promise((resolve) => helper.once('exit', resolve));
}

function finish() {
  finished = true;
  clearTimeout(timeout);
  if (stderr.trim()) {
    console.warn(`Text input helper stderr: ${stderr.trim()}`);
  }
  console.log(`Text input helper smoke check passed: ${path.relative(resolveProjectPath(), helperPath)}`);
}

function fail(message) {
  if (finished) return;
  finished = true;
  clearTimeout(timeout);
  try {
    helper.kill();
  } catch {
    // Ignore cleanup failures after a smoke failure.
  }
  console.error(message);
  if (stderr.trim()) {
    console.error(`stderr: ${stderr.trim()}`);
  }
  process.exit(1);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
