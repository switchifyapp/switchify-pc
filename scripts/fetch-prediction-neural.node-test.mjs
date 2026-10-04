import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { pinnedFile, bundlePins } from './fetch-prediction-neural.mjs';

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
