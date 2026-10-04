import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { pinnedFile, bundlePins, acquireWorker } from './fetch-prediction-neural.mjs';
import { zipSync } from 'fflate';

test('neural sources reject corrupt/oversized input and retain the previous cache', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'neural-source-'));
  try {
    const path = join(dir, 'source');
    const good = Buffer.from('good');
    const pin = { bytes: 4, sha256: createHash('sha256').update(good).digest('hex') };
    await writeFile(path, 'old');
    for (const bad of ['evil', '', 'toolong']) {
      await assert.rejects(pinnedFile(path, pin, 'https://fixture.invalid', async () => new Response(bad)));
      assert.equal(await readFile(path, 'utf8'), 'old');
    }
    await pinnedFile(path, pin, 'https://fixture.invalid', async () => new Response(good));
    await pinnedFile(path, pin, 'https://fixture.invalid', () => { throw new Error('cache should avoid download'); });
    assert.equal(await readFile(path, 'utf8'), 'good');
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('installed model pins retain the approved Q8 identity and license', () => {
  assert.equal(bundlePins['model.gguf'].size, 143041952);
  assert.equal(bundlePins['model.gguf'].sha256, '8d75e9b96c4b64e8a1180224cbaefbbc7744f21ca2e0be2319a429f2c589d342');
  assert.ok(bundlePins['MODEL_LICENSE.txt']);
  assert.ok(bundlePins['MODEL_CARD.md']);
});

test('prepared workers require pinned archive bytes and support verified offline reuse', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'neural-prepared-'));
  const hash = bytes => createHash('sha256').update(bytes).digest('hex');
  const bytes = Buffer.from('worker fixture');
  const zip = zipSync({ 'bundle/worker': bytes });
  const pin = { prepared_artifact: { run_id: 1, name: 'neural-windows-latest', archive: 'workers.zip' },
    archive_sha256: hash(zip), files: { 'bundle/worker': { size: bytes.length, sha256: hash(bytes) } } };
  const target = join(dir, 'workers');
  const runner = contents => async (command, args) => {
    assert.equal(command, 'gh');
    assert.equal(args[3], '--repo');
    await writeFile(join(args.at(-1), 'workers.zip'), contents);
  };
  try {
    await assert.rejects(acquireWorker(target, pin, dir, runner(Buffer.from('corrupt'))));
    await acquireWorker(target, pin, dir, runner(zip));
    assert.equal(await readFile(join(target, 'bundle/worker'), 'utf8'), 'worker fixture');
    await acquireWorker(target, pin, dir, () => { throw new Error('offline cache was not used'); });
    await assert.rejects(acquireWorker(join(dir, 'other'), { ...pin,
      prepared_artifact: { ...pin.prepared_artifact, archive: '../bad.zip' } }, dir, runner(zip)));
  } finally { await rm(dir, { recursive: true, force: true }); }
});
