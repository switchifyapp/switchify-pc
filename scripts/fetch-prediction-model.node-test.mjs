import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { zipSync, strToU8 } from 'fflate';
import { acquire, unpack, verified } from './fetch-prediction-model.mjs';
const sha = b => createHash('sha256').update(b).digest('hex');
function fixture(entries = { 'english.sqlite': strToU8('synthetic database'), 'corpus-notices/license': strToU8('test notice') }) {
  const archive = zipSync(entries);
  const files = Object.fromEntries(Object.entries(entries).filter(([n]) => !n.startsWith('../')).map(([n,b]) => [n, {sha256: sha(b), size:b.length}]));
  return {archive, manifest:{url:'https://example.invalid/model', archive_sha256:sha(archive), files}};
}
test('verified cache works offline and corrupt cache is replaced with verified files only', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'switchify-model-'));
  const target = join(dir, 'model');
  const {archive,manifest} = fixture();
  let requests = 0;
  const fetcher = async () => { requests++; return new Response(archive); };
  try {
    assert.equal(await acquire(target, manifest, fetcher), true);
    assert.equal(await verified(target, manifest), true);
    assert.equal(await acquire(target, manifest, () => { throw Error('offline'); }), false);
    assert.equal(requests, 1);
    await writeFile(join(target,'english.sqlite'), 'corrupt');
    assert.equal(await acquire(target, manifest, fetcher), true);
    assert.equal(await verified(target, manifest), true);
    assert.deepEqual(await readdir(dir), ['model']);
  } finally { await rm(dir, {recursive:true, force:true}); }
});
test('corrupt and interrupted downloads do not replace existing resources or leave staging files', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'switchify-model-'));
  const target = join(dir,'model');
  const {archive,manifest} = fixture();
  try {
    await acquire(target,manifest,async()=>new Response(archive));
    await writeFile(join(target,'english.sqlite'), 'old cache');
    await assert.rejects(acquire(target,manifest,async()=>new Response('bad')), /checksum/);
    await assert.rejects(acquire(target,manifest,async()=>new Response(new ReadableStream({start(c){c.enqueue(archive.subarray(0,8));c.error(Error('interrupted'));}}))), /interrupted/);
    assert.equal(await readFile(join(target,'english.sqlite'),'utf8'),'old cache');
    assert.deepEqual(await readdir(dir),['model']);
  } finally { await rm(dir,{recursive:true,force:true}); }
});
test('archive paths, missing files, and per-file tampering fail closed', () => {
  for (const path of ['../escape','/absolute','C:/absolute','folder\\escape','folder/../escape']) {
    const {archive,manifest}=fixture({'english.sqlite':strToU8('db'),[path]:strToU8('bad')});
    assert.throws(()=>unpack(archive,manifest), /Unsafe/);
  }
  const {archive,manifest}=fixture();
  manifest.files['missing.txt']={size:1,sha256:sha(strToU8('x'))};
  assert.throws(()=>unpack(archive,manifest), /checksum/);
  delete manifest.files['missing.txt'];
  manifest.files['english.sqlite'].sha256='0'.repeat(64);
  assert.throws(()=>unpack(archive,manifest), /checksum/);
});
test('unlisted files are never extracted', () => {
  const {archive,manifest}=fixture({'english.sqlite':strToU8('db'),'unused.txt':strToU8('extra')});
  delete manifest.files['unused.txt'];
  assert.deepEqual(Object.keys(unpack(archive,manifest)),['english.sqlite']);
});
