import test from 'node:test';
import assert from 'node:assert/strict';
import { createPrivateKey, createPublicKey, verify } from 'node:crypto';
import { readFile, mkdtemp, stat, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { issueLicence } from '../tools/licence-keygen/licence.mjs';

// Public test-only seed. Never a production trust anchor.
const testPem = createPrivateKey({ key: Buffer.from('302e020100300506032b657004220420' + '07'.repeat(32), 'hex'), type: 'pkcs8', format: 'der' }).export({ type: 'pkcs8', format: 'pem' });

test('Node issuance matches the frozen Rust compatibility vector', async () => {
  const vector = JSON.parse(await readFile(new URL('../tools/licence-keygen/test-vector.json', import.meta.url)));
  const result = issueLicence(testPem, { keyId: 'test', licenceId: '6af44000-17b7-4d87-ad1d-32e9a31b331a', issuedAt: 1000, expiresAt: 2000 });
  assert.equal(result.key, vector.key);
  const [prefix, kid, payload, signature] = result.key.split('.');
  assert(verify(null, Buffer.from(`${prefix}.${kid}.${payload}`), createPublicKey(testPem), Buffer.from(signature, 'base64url')));
  assert.throws(() => issueLicence(testPem, { keyId: 'test', seats: 0 }));
  assert.throws(() => issueLicence(testPem, { keyId: 'bad.key' }));
});

test('batch CLI produces unique keys, protects output, and never overwrites', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'switchify-licence-test-'));
  const cli = resolve('tools/licence-keygen/cli.mjs');
  const run = (...args) => spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' });
  try {
    const privatePath = join(dir, 'signing.pem');
    const publicPath = join(dir, 'verify.pem');
    const csvPath = join(dir, 'batch.csv');
    assert.equal(run('init', '--private', privatePath, '--public', publicPath).status, 0);
    const original = await readFile(privatePath);
    assert.notEqual(run('init', '--private', privatePath, '--public', publicPath).status, 0);
    assert.deepEqual(await readFile(privatePath), original);
    const args = ['batch', '--private', privatePath, '--key-id', 'partner-2026', '--out', csvPath, '--count', '3'];
    const generated = run(...args);
    assert.equal(generated.status, 0, generated.stderr);
    assert.equal(generated.stdout, '');
    const csv = await readFile(csvPath, 'utf8');
    const rows = csv.trim().split('\n').slice(1);
    assert.equal(new Set(rows.map(row => row.split(',')[0])).size, 3);
    for (const row of rows) {
      const key = row.split(',')[1];
      const parts = key.split('.');
      assert(verify(null, Buffer.from(parts.slice(0, 3).join('.')), await readFile(publicPath), Buffer.from(parts[3], 'base64url')));
    }
    if (process.platform !== 'win32') {
      assert.equal((await stat(csvPath)).mode & 0o777, 0o600);
      assert.equal((await stat(privatePath)).mode & 0o777, 0o600);
    }
    assert.notEqual(run(...args).status, 0);
    assert.equal(await readFile(csvPath, 'utf8'), csv);
    // Existing public key must not leave a newly generated orphan private key.
    const otherPrivate = join(dir, 'other.pem');
    await writeFile(publicPath, 'existing public file');
    assert.notEqual(run('init', '--private', otherPrivate, '--public', publicPath).status, 0);
    await assert.rejects(stat(otherPrivate), { code: 'ENOENT' });
    assert.equal(await readFile(publicPath, 'utf8'), 'existing public file');
  } finally { await rm(dir, { recursive: true, force: true }); }
});
