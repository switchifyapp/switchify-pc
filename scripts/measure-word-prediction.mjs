// Manual, read-only fixture measurement. Never sends an Accept request.
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { performance } from 'node:perf_hooks';

const [executable, app, expectedWord] = process.argv.slice(2);
if (!executable || !app || !expectedWord) {
  console.error('Usage: node scripts/measure-word-prediction.mjs <executable> <app-label> <expected-word>');
  process.exit(1);
}
console.log('Focus a synthetic test field now. Sampling starts in five seconds. No input will be injected.');
await delay(5000);
const child = spawn(resolve(executable), [
  '--switchify-prediction-worker',
  resolve('src-tauri/resources/WordData2017051601.db'),
  '[]',
], { stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true });
let buffer = Buffer.alloc(0);
let pending;
let failed = false;
const fail = () => {
  failed = true;
  pending?.reject(new Error('Worker unavailable'));
  pending = undefined;
};
child.on('error', fail);
child.on('exit', fail);
child.stdin.on('error', fail);
child.stdout.on('data', chunk => {
  buffer = Buffer.concat([buffer, chunk]);
  if (buffer.length < 4) return;
  const length = buffer.readUInt32LE();
  if (!length || length > 16384 || buffer.length > length + 4) return fail();
  if (buffer.length !== length + 4) return;
  try {
    const response = JSON.parse(buffer.subarray(4).toString('utf8'));
    buffer = Buffer.alloc(0);
    const request = pending;
    pending = undefined;
    if (!request) return fail();
    request.resolve(response);
  } catch { fail(); }
});
async function query(generation) {
  if (failed) throw new Error('Worker unavailable');
  const bytes = Buffer.from(JSON.stringify({ Query: {
    generation, edit: null, reset: true, shift: false, caps: false,
  } }));
  const header = Buffer.alloc(4);
  header.writeUInt32LE(bytes.length);
  let timer;
  try {
    return await new Promise((resolve, reject) => {
      pending = { resolve, reject };
      timer = setTimeout(() => { fail(); child.kill(); }, 2000);
      child.stdin.write(Buffer.concat([header, bytes]));
    });
  } finally { clearTimeout(timer); }
}
const stop = () => { fail(); child.kill(); };
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
const times = [];
let unsupported = 0;
let failures = 0;
let fixtureMatches = 0;
let firstRequestMs;
try {
  // Up to 200 attempts, stopping after 100 supported samples.
  for (let generation = 0; generation < 200 && times.length < 100; generation++) {
    const start = performance.now();
    const response = await query(generation);
    const elapsed = performance.now() - start;
    if (generation === 0) firstRequestMs = elapsed;
    const result = response.Suggestions;
    if (!result || result.generation !== generation) throw new Error('Invalid response');
    if (result.batch?.words?.length) {
      if (generation !== 0) times.push(elapsed);
      if (result.batch.words.some(word => word.toLowerCase() === expectedWord.toLowerCase())) fixtureMatches++;
    } else unsupported++;
    await delay(Math.max(0, 250 - elapsed));
  }
} catch { failures++; }
finally {
  child.kill();
  process.off('SIGINT', stop);
  process.off('SIGTERM', stop);
}
times.sort((a, b) => a - b);
const percentile = fraction => times.length ? +times[Math.ceil(times.length * fraction) - 1].toFixed(2) : null;
console.log(JSON.stringify({
  app, successfulSamples: times.length, unsupported, failures, fixtureMatches,
  firstRequestMs: firstRequestMs === undefined ? null : +firstRequestMs.toFixed(2),
  medianMs: percentile(0.5), p95Ms: percentile(0.95), maximumMs: percentile(1),
}, null, 2));
if (failures || times.length < 100) process.exitCode = 1;
