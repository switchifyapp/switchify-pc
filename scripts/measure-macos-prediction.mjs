// Manual read-only fixture measurement through Launch Services. Never sends Accept.
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, copyFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { performance } from 'node:perf_hooks';
import { setTimeout as delay } from 'node:timers/promises';

const [bundle, resource] = process.argv.slice(2);
if (process.platform !== 'darwin' || !bundle || !resource) {
  throw Error('Usage on macOS: node scripts/measure-macos-prediction.mjs <app-bundle> <lookup-path>');
}
console.error('Focus a synthetic test field. Sampling starts in five seconds; no input is injected.');
const root = mkdtempSync(join(tmpdir(), 'switchify-timing-'));
const input = join(root, 'input');
const output = join(root, 'output');
const lookup = join(root, 'timing.lookup');
const owned = new Set();
let child;
try {
  copyFileSync(resource, lookup);
  const frames = [];
  for (let generation = 0; generation < 100; generation++) {
    const body = Buffer.from(JSON.stringify({ Query: {
      generation, revision: 0, edits: [], shift: false, caps: false,
    } }));
    const header = Buffer.alloc(4);
    header.writeUInt32LE(body.length);
    frames.push(header, body);
  }
  writeFileSync(input, Buffer.concat(frames));
  writeFileSync(output, '');
  await delay(5000);
  const start = performance.now();
  child = spawn('open', ['-n', '-g', '-W', '-a', resolve(bundle),
    '--stdin', input, '--stdout', output, '--stderr', '/dev/null',
    '--args', '--switchify-prediction-worker', lookup, '[]']);
  let done = false;
  let launchError;
  child.on('exit', () => { done = true; });
  child.on('error', error => { launchError = error; done = true; });
  let offset = 0;
  let count = 0;
  let matched = 0;
  let firstResponseMs;
  let firstSuggestionsMs;
  let lastResponseMs;
  let maxSampledRssBytes = 0;
  while (performance.now() - start < 10000) {
    if (launchError) throw launchError;
    const data = readFileSync(output);
    while (offset + 4 <= data.length) {
      const size = data.readUInt32LE(offset);
      if (!size || size > 16384) throw Error('Invalid frame');
      if (offset + 4 + size > data.length) break;
      const response = JSON.parse(data.subarray(offset + 4, offset + 4 + size));
      offset += 4 + size;
      if (response.Suggestions?.generation !== count) throw Error('Unexpected response');
      firstResponseMs ??= performance.now() - start;
      if (response.Suggestions.batch?.words?.length) {
        firstSuggestionsMs ??= performance.now() - start;
        matched++;
      }
      count++;
      lastResponseMs = performance.now() - start;
    }
    if (done) break;
    try {
      const rows = execFileSync('ps', ['-axo', 'pid=,rss=,command='], { encoding: 'utf8' }).split('\n');
      for (const row of rows) {
        const match = row.trim().match(/^(\d+)\s+(\d+)\s+(.+)$/);
        if (match && match[3].includes('--switchify-prediction-worker') &&
            match[3].includes(lookup) && !/^(?:\/usr\/bin\/)?open /.test(match[3])) {
          owned.add(Number(match[1]));
          maxSampledRssBytes = Math.max(maxSampledRssBytes, Number(match[2]) * 1024);
        }
      }
    } catch { /* A worker may exit between samples. */ }
    await delay(10);
  }
  console.log(JSON.stringify({
    firstResponseMs, firstSuggestionsMs, count, matched,
    maxSampledRssBytes: maxSampledRssBytes || null,
    warmBatchMeanObservedMs: count > 1 ? (lastResponseMs - firstResponseMs) / (count - 1) : null,
    elapsedMs: performance.now() - start,
  }, null, 2));
  if (count !== 100 || !matched || firstSuggestionsMs >= 2000) process.exitCode = 1;
} finally {
  for (const pid of owned) {
    try {
      const command = execFileSync('ps', ['-p', String(pid), '-o', 'command='], { encoding: 'utf8' });
      if (command.includes(lookup)) process.kill(pid);
    } catch { /* Already exited. */ }
  }
  child?.kill();
  rmSync(root, { recursive: true, force: true });
}
