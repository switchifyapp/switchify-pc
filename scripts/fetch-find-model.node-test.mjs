import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { acquire } from './fetch-find-model.mjs';
import { packModel } from '../src/find/semantic.ts';
const meta = b => ({ size: b.length, sha256: createHash('sha256').update(b).digest('hex') });
async function fixture() {
  const tokenizer = Buffer.from(JSON.stringify({model: {vocab: {'[UNK]': 0, word: 1}}}));
  const header = Buffer.from(JSON.stringify({embeddings: {dtype: 'F32', shape: [2, 2], data_offsets: [0, 16]}}));
  const tensor = Buffer.alloc(8 + header.length + 16); tensor.writeBigUInt64LE(BigInt(header.length)); header.copy(tensor, 8); tensor.writeFloatLE(1, 8 + header.length + 8);
  const inputs = {'tokenizer.json': tokenizer, 'model.safetensors': tensor};
  const outputs = {'find-model.bin': packModel(['[UNK]', 'word'], 2, new Float32Array([0, 0, 1, 0]))};
  for (const name of ['LICENSE', 'NOTICE']) outputs[name] = await readFile(new URL(`./find-model.${name}`, import.meta.url));
  return { inputs, manifest: {url: 'https://example.invalid', dims: 2, sources: Object.fromEntries(Object.entries(inputs).map(([n,b])=>[n,meta(b)])), files: Object.fromEntries(Object.entries(outputs).map(([n,b])=>[n,meta(b)]))} };
}
test('find acquisition verifies outputs and skips network when cached', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'switchify-find-test-'));
  try {
    const {inputs,manifest}=await fixture(); const target=join(directory,'model');
    const fetcher = async url => new Response(inputs[url.split('/').at(-1)]);
    assert.equal(await acquire(target,manifest,fetcher),true);
    assert.equal(await acquire(target,manifest,()=>{throw Error('offline');}),false);
    assert.equal((await readFile(join(target,'find-model.bin'))).length,manifest.files['find-model.bin'].size);
  } finally {await rm(directory,{recursive:true,force:true});}
});
test('unverified and oversized input never creates the destination', async () => {
  const directory=await mkdtemp(join(tmpdir(),'switchify-find-test-'));
  try {
    const {manifest}=await fixture(); const target=join(directory,'model');
    await assert.rejects(acquire(target,manifest,async()=>new Response('bad')),/checksum/);
    await assert.rejects(acquire(target,manifest,async()=>new Response(Buffer.alloc(1024))),/too large/);
    await assert.rejects(access(target));
  } finally {await rm(directory,{recursive:true,force:true});}
});
