import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm, stat, writeFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { zipSync, strToU8 } from 'fflate';
import { acquireRuntime, buildTarget, isCurrent, manifest, renameWithRetry, runtimeTarget, unpackRuntime } from './fetch-textslinger-runtime.mjs';
import { rename } from 'node:fs/promises';
import { frame, frames } from './check-textslinger-runtime.mjs';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');

function archive(files, target = 'x86_64-pc-windows-msvc') {
  const description = { format_version: 1, target, python: 'python.exe', model: 'model/m.kenlm', model_alphabet: 'ab ' };
  const zip = Buffer.from(zipSync({
    'manifest.json': strToU8(JSON.stringify(description)),
    'python.exe': strToU8('interpreter'),
    'model/m.kenlm': strToU8('model'),
    ...files,
  }));
  return { zip, pin: { url: 'https://fixture.invalid/runtime.zip', bytes: zip.length, sha256: sha(zip) } };
}

const pinsFor = pin => ({ targets: { 'x86_64-pc-windows-msvc': pin }, runtime_for: {} });

test('pins name a v0.6 release asset for every shipped target', () => {
  for (const target of ['x86_64-pc-windows-msvc', 'aarch64-apple-darwin', 'x86_64-apple-darwin']) {
    const pin = manifest.targets[target];
    assert.match(pin.url, new RegExp(`/releases/download/${manifest.release}/textslinger-runtime-${target}\\.zip$`));
    assert.match(pin.sha256, /^[0-9a-f]{64}$/);
    assert.ok(Number.isSafeInteger(pin.bytes) && pin.bytes > 0);
  }
});

test('Windows ARM64 bundles the x64 runtime; unsupported targets fail', () => {
  assert.equal(runtimeTarget('aarch64-pc-windows-msvc'), 'x86_64-pc-windows-msvc');
  assert.equal(runtimeTarget('x86_64-pc-windows-msvc'), 'x86_64-pc-windows-msvc');
  assert.equal(runtimeTarget('aarch64-apple-darwin'), 'aarch64-apple-darwin');
  assert.throws(() => runtimeTarget('x86_64-unknown-linux-gnu'));
});

test('the build target comes from Tauri, then from rustc', () => {
  assert.equal(buildTarget({ TAURI_ENV_TARGET_TRIPLE: 'aarch64-pc-windows-msvc' }), 'aarch64-pc-windows-msvc');
  const rustc = () => ({ status: 0, stdout: 'release: 1.97.1\nhost: aarch64-apple-darwin\n' });
  assert.equal(buildTarget({}, rustc), 'aarch64-apple-darwin');
  assert.throws(() => buildTarget({}, () => ({ status: 1, stdout: '' })));
});

test('archives must match the pin, stay inside the runtime and describe themselves', () => {
  const { zip, pin } = archive({});
  assert.equal(unpackRuntime(zip, pin, 'x86_64-pc-windows-msvc').description.python, 'python.exe');
  const tampered = Buffer.from(zip);
  tampered[tampered.length - 1] ^= 1;
  assert.throws(() => unpackRuntime(tampered, pin, 'x86_64-pc-windows-msvc'), /checksum/);
  for (const name of ['../escape', '/absolute', 'a\\b', 'c:/drive', 'a/./b']) {
    const bad = archive({ [name]: strToU8('x') });
    assert.throws(() => unpackRuntime(bad.zip, bad.pin, 'x86_64-pc-windows-msvc'), /Unsafe/, name);
  }
  const directory = archive({ 'site/': new Uint8Array() });
  assert.throws(() => unpackRuntime(directory.zip, directory.pin, 'x86_64-pc-windows-msvc'), /Unsafe/);
  const libraries = archive({ 'numpy/core/lib/npymath.lib': strToU8('lib'), 'numpy/core/lib/libnpymath.a': strToU8('a') });
  const kept = Object.keys(unpackRuntime(libraries.zip, libraries.pin, 'x86_64-pc-windows-msvc').entries).sort();
  assert.deepEqual(kept, ['manifest.json', 'model/m.kenlm', 'python.exe']);
  const other = archive({}, 'aarch64-apple-darwin');
  assert.throws(() => unpackRuntime(other.zip, other.pin, 'x86_64-pc-windows-msvc'), /manifest/);
});

test('acquiring writes the runtime once and keeps it when a new download is wrong', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'textslinger-runtime-'));
  try {
    const destination = join(dir, 'runtime');
    const { zip, pin } = archive({});
    const pins = pinsFor(pin);
    assert.equal(await acquireRuntime(destination, 'x86_64-pc-windows-msvc', pins, async () => new Response(zip)), true);
    assert.equal(await readFile(join(destination, 'model/m.kenlm'), 'utf8'), 'model');
    assert.equal(await isCurrent(destination, pin), true);
    assert.equal(await acquireRuntime(destination, 'x86_64-pc-windows-msvc', pins,
      () => { throw new Error('a current runtime must not be downloaded again'); }), false);

    const next = archive({ 'extra.txt': strToU8('new') });
    await assert.rejects(acquireRuntime(destination, 'x86_64-pc-windows-msvc', pinsFor(next.pin),
      async () => new Response(Buffer.concat([next.zip, Buffer.from('x')]))));
    assert.equal(await readFile(join(destination, 'model/m.kenlm'), 'utf8'), 'model');
    await assert.rejects(stat(join(destination, 'extra.txt')));
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('a failed swap restores the previous runtime, and leftovers are removed', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'textslinger-runtime-'));
  try {
    const destination = join(dir, 'runtime');
    const first = archive({});
    await acquireRuntime(destination, 'x86_64-pc-windows-msvc', pinsFor(first.pin), async () => new Response(first.zip));
    const next = archive({ 'extra.txt': strToU8('new') });
    // Moving the old runtime aside works; moving the new one into place does not.
    const failing = async (from, to) => {
      if (to === destination) throw Object.assign(new Error('disk full'), { code: 'ENOSPC' });
      return rename(from, to);
    };
    await assert.rejects(acquireRuntime(destination, 'x86_64-pc-windows-msvc', pinsFor(next.pin),
      async () => new Response(next.zip), failing), /disk full/);
    assert.equal(await readFile(join(destination, 'model/m.kenlm'), 'utf8'), 'model');
    await assert.rejects(stat(join(destination, 'extra.txt')));

    await mkdir(join(dir, 'runtime.pending-leftover'));
    await acquireRuntime(destination, 'x86_64-pc-windows-msvc', pinsFor(next.pin), async () => new Response(next.zip));
    assert.equal(await readFile(join(destination, 'extra.txt'), 'utf8'), 'new');
    await assert.rejects(stat(join(dir, 'runtime.pending-leftover')));
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('renames are retried while a scanner holds the folder', async () => {
  let calls = 0;
  const flaky = async () => {
    calls++;
    if (calls < 3) throw Object.assign(new Error('busy'), { code: 'EPERM' });
  };
  await renameWithRetry('a', 'b', 5, flaky);
  assert.equal(calls, 3);
  await assert.rejects(renameWithRetry('a', 'b', 5, async () => {
    throw Object.assign(new Error('gone'), { code: 'ENOENT' });
  }), /gone/);
});

test('a runtime without its stamp or manifest is not current', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'textslinger-runtime-'));
  try {
    const destination = join(dir, 'runtime');
    const pin = { sha256: 'a'.repeat(64) };
    await mkdir(destination);
    await writeFile(`${destination}.sha256`, `${pin.sha256} unpack-2\n`);
    assert.equal(await isCurrent(destination, pin), false);
    await writeFile(join(destination, 'manifest.json'), '{}');
    assert.equal(await isCurrent(destination, pin), true);
    assert.equal(await isCurrent(destination, { sha256: 'b'.repeat(64) }), false);
    // A copy unpacked by an earlier version of the fetcher is refreshed.
    await writeFile(`${destination}.sha256`, `${pin.sha256}\n`);
    assert.equal(await isCurrent(destination, pin), false);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('bridge frames round-trip across chunk boundaries and reject bad lengths', () => {
  const bytes = Buffer.concat([frame({ ready: true }), frame({ id: 1, words: ['door'] })]);
  const decode = frames();
  const decoded = [];
  for (const byte of bytes) decoded.push(...decode(Buffer.from([byte])));
  assert.deepEqual(decoded, [{ ready: true }, { id: 1, words: ['door'] }]);
  assert.throws(() => frames()(Buffer.from([0, 0, 0, 0])), /Invalid frame/);
});
