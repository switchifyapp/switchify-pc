// Build-time acquisition. The installed app only reads its bundled asset.
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, mkdtemp, rename, rm } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { packModel } from '../src/find/semantic.ts';
import { vocabularyFromTokenizerJson } from '../src/find/wordpiece.ts';
import { verified } from './fetch-prediction-model.mjs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
export async function acquire(target, manifest, fetcher = fetch) {
  if (await verified(target, manifest)) return false;
  const inputs = {};
  for (const [name, expected] of Object.entries(manifest.sources)) {
    const response = await fetcher(`${manifest.url}/${name}`, { signal: AbortSignal.timeout(120000) });
    if (!response.ok || !response.body) throw new Error(`Find model download failed: HTTP ${response.status}`);
    const chunks = []; let size = 0;
    for await (const chunk of response.body) {
      size += chunk.length;
      if (size > expected.size || size > 16 * 1024 * 1024) throw new Error('Find model download too large');
      chunks.push(chunk);
    }
    const bytes = Buffer.concat(chunks);
    if (bytes.length !== expected.size || hash(bytes) !== expected.sha256) throw new Error('Find model checksum mismatch');
    inputs[name] = bytes;
  }
  const vocab = vocabularyFromTokenizerJson(JSON.parse(inputs['tokenizer.json']));
  const bytes = inputs['model.safetensors'];
  const length = Number(bytes.readBigUInt64LE());
  const header = JSON.parse(bytes.subarray(8, 8 + length));
  const tensor = header.embeddings;
  if (tensor.dtype !== 'F32' || tensor.shape[0] !== vocab.size || tensor.shape[1] !== manifest.dims) throw new Error('Unexpected find model shape');
  const start = 8 + length + tensor.data_offsets[0];
  const rows = new Float32Array(bytes.buffer.slice(bytes.byteOffset + start, bytes.byteOffset + start + vocab.size * manifest.dims * 4));
  const words = [...vocab.entries()].sort((a, b) => a[1] - b[1]).map(([word]) => word);
  // Row i of the matrix is token i, so the ids must be exactly 0..n-1.
  words.forEach((word, i) => { if (vocab.get(word) !== i) throw new Error('Find model vocabulary ids are not contiguous'); });
  const files = { 'find-model.bin': packModel(words, manifest.dims, rows) };
  for (const name of ['LICENSE', 'NOTICE']) files[name] = await readFile(new URL(`./find-model.${name}`, import.meta.url));
  for (const [name, data] of Object.entries(files)) {
    if (data.length !== manifest.files[name].size || hash(data) !== manifest.files[name].sha256) throw new Error('Find model output checksum mismatch');
  }
  // Only verified inputs and outputs reach disk, using a staged directory swap.
  await mkdir(dirname(target), { recursive: true });
  const staging = await mkdtemp(`${target}.pending-`);
  const previous = `${staging}.previous`;
  let moved = false;
  try {
    for (const [name, data] of Object.entries(files)) await writeFile(join(staging, name), data);
    try { await rename(target, previous); moved = true; }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    try { await rename(staging, target); }
    catch (error) { if (moved) await rename(previous, target); throw error; }
    await rm(previous, { recursive: true, force: true });
    return true;
  } finally { await rm(staging, { recursive: true, force: true }); }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const manifest = JSON.parse(await readFile(new URL('./find-model.json', import.meta.url)));
  if (await acquire(fileURLToPath(new URL('../src/find/model', import.meta.url)), manifest)) console.log(`Fetched and verified ${manifest.model}`);
}
