// Build-time acquisition only. The installed app never downloads prediction data.
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, mkdtemp, rename, rm, lstat } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { unzipSync } from 'fflate';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const MAX_ARCHIVE = 64 * 1024 * 1024;

export async function verified(directory, manifest) {
  try {
    for (const [name, expected] of Object.entries(manifest.files)) {
      const path = join(directory, name);
      const info = await lstat(path);
      if (!info.isFile() || info.size !== expected.size || digest(await readFile(path)) !== expected.sha256) return false;
    }
    return true;
  } catch { return false; }
}

export function unpack(bytes, manifest) {
  if (bytes.length > MAX_ARCHIVE || digest(bytes) !== manifest.archive_sha256) throw new Error('Prediction archive checksum mismatch');
  const seen = new Set();
  const entries = unzipSync(new Uint8Array(bytes), { filter: entry => {
    const name = entry.name;
    if (name.startsWith('/') || name.includes('\\') || name.includes(':') || name.split('/').some(p => p === '..' || p === '.') || seen.has(name)) throw new Error('Unsafe prediction archive path');
    seen.add(name);
    if (!Object.hasOwn(manifest.files, name)) return false;
    const expected = manifest.files[name];
    if (entry.originalSize !== expected.size) throw new Error('Prediction entry size mismatch');
    return true;
  }});
  for (const [name, expected] of Object.entries(manifest.files)) {
    if (!entries[name] || entries[name].length !== expected.size || digest(entries[name]) !== expected.sha256) throw new Error('Prediction file checksum mismatch');
  }
  return entries;
}

async function download(url, fetcher) {
  const response = await fetcher(url, { signal: AbortSignal.timeout(120000) });
  if (!response.ok || !response.body) throw new Error(`Prediction download failed: HTTP ${response.status}`);
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > MAX_ARCHIVE) throw new Error('Prediction archive too large');
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

export async function acquire(target, manifest, fetcher = fetch) {
  if (await verified(target, manifest)) return false;
  // No destination writes until the complete archive and every selected file verify.
  const entries = unpack(await download(manifest.url, fetcher), manifest);
  await mkdir(dirname(target), { recursive: true });
  const staging = await mkdtemp(`${target}.pending-`);
  const previous = `${staging}.previous`;
  let moved = false;
  try {
    for (const [name, bytes] of Object.entries(entries)) {
      await mkdir(dirname(join(staging, name)), { recursive: true });
      await writeFile(join(staging, name), bytes);
    }
    if (!(await verified(staging, manifest))) throw new Error('Prediction staging verification failed');
    try { await rename(target, previous); moved = true; }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    try { await rename(staging, target); }
    catch (error) { if (moved) await rename(previous, target); throw error; }
    await rm(previous, { recursive: true, force: true });
    return true;
  } finally { await rm(staging, { recursive: true, force: true }); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const manifest = JSON.parse(await readFile(new URL('./prediction-model.json', import.meta.url), 'utf8'));
  const target = fileURLToPath(new URL('../src-tauri/resources/prediction-model', import.meta.url));
  if (await acquire(target, manifest)) console.log(`Fetched and verified ${manifest.model}`);
}
