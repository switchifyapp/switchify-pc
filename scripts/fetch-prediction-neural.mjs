// Build-time only. Installed applications never fetch models or workers.
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, copyFile, chmod, mkdtemp, rm } from 'node:fs/promises';
import { join, resolve, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { acquire, verified } from './fetch-prediction-model.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const manifest = JSON.parse(await readFile(new URL('./prediction-neural.json', import.meta.url), 'utf8'));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
export const bundlePins = Object.fromEntries(Object.entries(manifest.bundle.files).map(([name, pin]) =>
  [name, { size: pin.bytes, sha256: pin.sha256 }]));

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit', windowsHide: true });
  if (result.error || result.status !== 0) throw new Error(`Neural asset preparation failed: ${command}`);
}

export async function pinnedFile(path, pin, url, fetcher = fetch) {
  try {
    const bytes = await readFile(path);
    if (bytes.length === pin.bytes && hash(bytes) === pin.sha256) return;
  } catch { /* Missing cache entry. */ }
  const response = await fetcher(url, { signal: AbortSignal.timeout(300000) });
  if (!response.ok || !response.body) throw new Error('Neural source download failed');
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > pin.bytes) throw new Error('Neural source exceeds pinned size');
    chunks.push(chunk);
  }
  const bytes = Buffer.concat(chunks);
  if (size !== pin.bytes || hash(bytes) !== pin.sha256) throw new Error('Neural source checksum mismatch');
  await writeFile(path, bytes);
}

// Prepared companion artifacts are review inputs, not a published release.
// Only the pinned archive and selected file hashes are trusted. A later release
// replaces this source with its permanent public asset URL without changing bytes.
export async function acquireWorker(workers, pin, cache, runner = run) {
  if (!pin.prepared_artifact) return acquire(workers, pin);
  if (await verified(workers, pin)) return false;
  const prepared = pin.prepared_artifact;
  if (!Number.isSafeInteger(prepared.run_id) || prepared.run_id <= 0 ||
      !/^neural-[a-z0-9-]+$/.test(prepared.name) ||
      basename(prepared.archive) !== prepared.archive || !prepared.archive.endsWith('.zip')) {
    throw new Error('Invalid prepared worker source');
  }
  await mkdir(cache, { recursive: true });
  const staging = await mkdtemp(join(cache, 'prepared-'));
  try {
    await runner('gh', ['run', 'download', String(prepared.run_id), '--repo',
      'switchifyapp/switchify-prediction', '--name', prepared.name, '--dir', staging], root);
    const bytes = await readFile(join(staging, prepared.archive));
    return await acquire(workers, { ...pin, url: 'prepared-worker' }, async () => new Response(bytes));
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
}

export async function prepare() {
  const target = process.env.TAURI_ENV_TARGET_TRIPLE ??
    (process.platform === 'win32' ? 'x86_64-pc-windows-msvc' :
      process.platform === 'darwin' ? `${process.arch === 'arm64' ? 'aarch64' : 'x86_64'}-apple-darwin` : 'x86_64-unknown-linux-gnu');
  const workerPin = manifest.workers[target];
  if (!workerPin) throw new Error(`Unsupported neural target: ${target}`);
  const cache = join(root, '.cache/prediction-neural');
  const bundle = join(root, 'src-tauri/resources/prediction-neural');
  const workers = join(cache, target);
  const binaries = join(root, 'src-tauri/binaries');
  await mkdir(bundle, { recursive: true });
  await mkdir(binaries, { recursive: true });
  await acquireWorker(workers, workerPin, cache);
  for (const name of Object.keys(workerPin.files)) {
    const file = basename(name);
    if (file.startsWith('switchify-smol-worker')) {
      // Only Windows uses the optional accelerated worker.
      if (file.includes('avx2') && process.platform !== 'win32') continue;
      const ext = file.endsWith('.exe') ? '.exe' : '';
      const stem = ext ? file.slice(0, -4) : file;
      const dest = join(binaries, `${stem}-${target}${ext}`);
      await copyFile(join(workers, name), dest);
      await chmod(dest, 0o755);
    } else {
      await mkdir(join(bundle, 'worker-notices'), { recursive: true });
      await copyFile(join(workers, name), join(bundle, 'worker-notices', file));
    }
  }
  if (!(await verified(bundle, { files: bundlePins }))) {
    const source = join(cache, 'source');
    await mkdir(source, { recursive: true });
    for (const [name, pin] of Object.entries(manifest.sources.files)) {
      await pinnedFile(join(source, name), pin, pin.url);
    }
    const upstream = join(cache, 'converter');
    await mkdir(upstream, { recursive: true });
    run('git', ['init', '--quiet'], upstream);
    run('git', ['fetch', '--quiet', '--depth=1', 'https://github.com/switchifyapp/switchify-prediction', manifest.revision], upstream);
    run('git', ['checkout', '--quiet', '--detach', 'FETCH_HEAD'], upstream);
    // Reject local converter modifications rather than executing them.
    run('git', ['diff', '--exit-code', 'HEAD', '--'], upstream);
    const model = join(cache, 'model.gguf');
    if (!(await verified(cache, { files: { 'model.gguf': bundlePins['model.gguf'] } }))) {
      // The converter refuses overwrites; a corrupt cache is reported explicitly.
      run('cargo', ['run', '--locked', '--release', '--manifest-path', join(upstream, 'neural/Cargo.toml'),
        '-p', 'switchify-smol-worker', '--bin', 'quantize', '--target-dir', join(cache, 'target'), '--', source, model], root);
    }
    if (!(await verified(cache, { files: { 'model.gguf': bundlePins['model.gguf'] } }))) throw new Error('Converted model checksum mismatch');
    await copyFile(model, join(bundle, 'model.gguf'));
    for (const [dest, from] of [['config.json','config.json'], ['tokenizer.json','tokenizer.json'], ['MODEL_CARD.md','README.md']]) {
      await copyFile(join(source, from), join(bundle, dest));
    }
    await copyFile(join(upstream, 'neural/MODEL_LICENSE.txt'), join(bundle, 'MODEL_LICENSE.txt'));
    await copyFile(join(upstream, 'neural/worker/src/bin/quantize.rs'), join(bundle, 'quantize.rs'));
  }
  await writeFile(join(bundle, 'model-bundle.json'), JSON.stringify(manifest.bundle, null, 2) + '\n');
  await writeFile(join(bundle, 'source-manifest.json'), JSON.stringify(manifest.sources, null, 2) + '\n');
  if (!(await verified(bundle, { files: bundlePins }))) throw new Error('Neural bundle verification failed');
  console.log(`Verified offline neural model and ${target} workers`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await prepare();
