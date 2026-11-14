// Runs the bundled TextSlinger runtime and bridge once, with fixed text, to
// check that they work where they are packaged. No desktop input is involved.
//
// Usage: node scripts/check-textslinger-runtime.mjs <folder containing textslinger/runtime>
import { spawn } from 'node:child_process';
import { readdir, readFile, stat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const LIMIT = 16384;

// Finds the folder holding textslinger/runtime/manifest.json and textslinger/bridge.py.
export async function locate(start, depth = 8) {
  const candidate = join(start, 'textslinger');
  try {
    await stat(join(candidate, 'runtime', 'manifest.json'));
    await stat(join(candidate, 'bridge.py'));
    return candidate;
  } catch { /* keep looking */ }
  if (depth === 0) return undefined;
  let children = [];
  try { children = await readdir(start, { withFileTypes: true }); } catch { return undefined; }
  for (const child of children) {
    if (child.isDirectory() && !child.isSymbolicLink()) {
      const found = await locate(join(start, child.name), depth - 1);
      if (found) return found;
    }
  }
  return undefined;
}

export function frame(value) {
  const payload = Buffer.from(JSON.stringify(value), 'utf8');
  const header = Buffer.alloc(4);
  header.writeUInt32LE(payload.length);
  return Buffer.concat([header, payload]);
}

// Yields decoded frames from a stream of chunks.
export function frames() {
  let buffer = Buffer.alloc(0);
  return chunk => {
    buffer = Buffer.concat([buffer, chunk]);
    const out = [];
    while (buffer.length >= 4) {
      const size = buffer.readUInt32LE(0);
      if (size === 0 || size > LIMIT) throw new Error('Invalid frame from bridge');
      if (buffer.length < 4 + size) break;
      out.push(JSON.parse(buffer.subarray(4, 4 + size).toString('utf8')));
      buffer = buffer.subarray(4 + size);
    }
    return out;
  };
}

export async function check(textslinger, { timeoutMs = 180000 } = {}) {
  const runtime = join(textslinger, 'runtime');
  const description = JSON.parse(await readFile(join(runtime, 'manifest.json'), 'utf8'));
  const python = join(runtime, description.python);
  const started = Date.now();
  const child = spawn(python, ['-I', '-B', join(textslinger, 'bridge.py'), '--runtime', runtime],
    // stderr is shown: the input is fixed text, and the bridge prints only
    // start-up errors, so a failing packaged runtime can say why.
    { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
  const decode = frames();
  const replies = [];
  let waiting;
  child.stdout.on('data', chunk => { replies.push(...decode(chunk)); waiting?.(); });
  const exited = new Promise(done => child.on('exit', code => { waiting?.(); done(code); }));
  const next = async (label) => {
    const deadline = Date.now() + timeoutMs;
    while (!replies.length) {
      if (child.exitCode !== null) throw new Error(`Bridge exited (${child.exitCode}) waiting for ${label}`);
      if (Date.now() > deadline) throw new Error(`Timed out waiting for ${label}`);
      await new Promise(done => { waiting = done; setTimeout(done, 250); });
    }
    return replies.shift();
  };
  try {
    const ready = await next('ready');
    if (ready?.ready !== true) throw new Error('Bridge did not report ready');
    const readyMs = Date.now() - started;
    child.stdin.write(frame({ id: 1, before: 'please open the ', prefix: '' }));
    const reply = await next('suggestions');
    if (reply?.id !== 1 || !Array.isArray(reply.words) || !reply.words.includes('door')) {
      throw new Error(`Unexpected suggestions: ${JSON.stringify(reply)}`);
    }
    child.stdin.write(frame({ shutdown: true }));
    const stopped = await next('shutdown');
    if (stopped?.stopped !== true) throw new Error('Bridge did not stop cleanly');
    child.stdin.end();
    if ((await exited) !== 0) throw new Error('Bridge exited with an error after shutdown');
    return { target: description.target, readyMs, words: reply.words };
  } finally {
    if (child.exitCode === null) child.kill();
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const start = resolve(process.argv[2] ?? join(fileURLToPath(new URL('../', import.meta.url)), 'src-tauri/resources'));
  const textslinger = await locate(start);
  if (!textslinger) {
    console.error(`No textslinger/runtime with bridge.py under ${start}`);
    process.exit(1);
  }
  try {
    const result = await check(textslinger);
    console.log(`TextSlinger ${result.target} ready in ${result.readyMs} ms; suggestions: ${result.words.join(', ')}`);
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}
