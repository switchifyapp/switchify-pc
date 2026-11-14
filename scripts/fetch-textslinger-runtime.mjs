// Build-time only. Installed applications never download the runtime.
//
// Fetches the TextSlinger runtime for the build target from a switchify-prediction
// release, pinned by size and SHA-256, into src-tauri/resources/textslinger/runtime.
import { createHash } from 'node:crypto';
import { chmod, mkdir, mkdtemp, readdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { unzipSync } from 'fflate';

const root = fileURLToPath(new URL('../', import.meta.url));
export const manifest = JSON.parse(await readFile(new URL('./textslinger-runtime.json', import.meta.url), 'utf8'));
export const runtimeDirectory = join(root, 'src-tauri/resources/textslinger/runtime');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

// The runtime a build target bundles. Windows ARM64 runs the x64 runtime
// under Windows' x64 emulation; there is no Python 3.10 build for ARM64.
export function runtimeTarget(target, pins = manifest) {
  const runtime = pins.runtime_for[target] ?? target;
  if (!pins.targets[runtime]) throw new Error(`No TextSlinger runtime for ${target}`);
  return runtime;
}

// Cargo builds for rustc's host unless Tauri names a target.
export function buildTarget(env = process.env, runner = spawnSync) {
  if (env.TAURI_ENV_TARGET_TRIPLE) return env.TAURI_ENV_TARGET_TRIPLE;
  const result = runner('rustc', ['-vV'], { encoding: 'utf8' });
  const host = result.status === 0 ? /^host: (\S+)$/m.exec(result.stdout ?? '')?.[1] : undefined;
  if (!host) throw new Error('Could not determine the Rust host target');
  return host;
}

async function download(url, pin, fetcher) {
  const response = await fetcher(url, { signal: AbortSignal.timeout(300000) });
  if (!response.ok || !response.body) throw new Error(`TextSlinger runtime download failed: HTTP ${response.status}`);
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > pin.bytes) throw new Error('TextSlinger runtime is larger than pinned');
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

// Static libraries for building extensions. The runtime never loads them, and
// leaving them out means no unsigned archives of Mach-O objects in the app.
const unused = name => /\.(a|lib)$/i.test(name);

// Checks the archive against its pin and returns its files. Nothing is written.
export function unpackRuntime(bytes, pin, runtime) {
  if (bytes.length !== pin.bytes || digest(bytes) !== pin.sha256) throw new Error('TextSlinger runtime checksum mismatch');
  const seen = new Set();
  const entries = unzipSync(new Uint8Array(bytes), { filter: entry => {
    const name = entry.name;
    if (name.startsWith('/') || name.includes('\\') || name.includes(':') || name.endsWith('/') ||
        name.split('/').some(part => part === '' || part === '..' || part === '.') || seen.has(name)) {
      throw new Error(`Unsafe TextSlinger runtime path: ${name}`);
    }
    seen.add(name);
    return !unused(name);
  }});
  const description = JSON.parse(Buffer.from(entries['manifest.json'] ?? []).toString('utf8') || 'null');
  if (description?.format_version !== 1 || description.target !== runtime ||
      !entries[description.python] || !entries[description.model]) {
    throw new Error('TextSlinger runtime manifest does not match the archive');
  }
  return { entries, description };
}

const stampFor = directory => `${directory}.sha256`;

export async function isCurrent(directory, pin) {
  try {
    const stamp = (await readFile(stampFor(directory), 'utf8')).trim();
    JSON.parse(await readFile(join(directory, 'manifest.json'), 'utf8'));
    return stamp === pin.sha256;
  } catch { return false; }
}

// Antivirus scanning of just-written files can briefly block a rename on Windows.
export async function renameWithRetry(from, to, attempts = 5, rename_ = rename) {
  for (let attempt = 1; ; attempt++) {
    try { return await rename_(from, to); }
    catch (error) {
      if (attempt >= attempts || !['EPERM', 'EBUSY', 'EACCES'].includes(error.code)) throw error;
      await new Promise(done => setTimeout(done, 200 * attempt));
    }
  }
}

// Removes staging folders that an interrupted run left behind.
export async function removeLeftovers(directory) {
  const prefix = `${basename(directory)}.pending-`;
  let names = [];
  try { names = await readdir(dirname(directory)); } catch { return; }
  for (const name of names) {
    if (name.startsWith(prefix)) await rm(join(dirname(directory), name), { recursive: true, force: true });
  }
}

// Replaces directory with the pinned runtime, all at once.
export async function acquireRuntime(directory, runtime, pins = manifest, fetcher = fetch, rename_ = rename) {
  const pin = pins.targets[runtime];
  await removeLeftovers(directory);
  if (await isCurrent(directory, pin)) return false;
  const { entries, description } = unpackRuntime(await download(pin.url, pin, fetcher), pin, runtime);
  await mkdir(dirname(directory), { recursive: true });
  const staging = await mkdtemp(`${directory}.pending-`);
  const previous = `${staging}.previous`;
  let moved = false;
  try {
    for (const [name, bytes] of Object.entries(entries)) {
      await mkdir(dirname(join(staging, name)), { recursive: true });
      await writeFile(join(staging, name), bytes);
    }
    // fflate does not carry file modes; the macOS interpreter must be executable.
    if (process.platform !== 'win32') await chmod(join(staging, description.python), 0o755);
    await rm(stampFor(directory), { force: true });
    try { await renameWithRetry(directory, previous, 5, rename_); moved = true; }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    try { await renameWithRetry(staging, directory, 5, rename_); }
    catch (error) { if (moved) await rename(previous, directory); throw error; }
    await rm(previous, { recursive: true, force: true });
    await writeFile(stampFor(directory), `${pin.sha256}\n`);
    return true;
  } finally { await rm(staging, { recursive: true, force: true }); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const target = buildTarget();
  if (target.includes('-linux-')) {
    // Switchify PC does not ship on Linux; this keeps other checks usable there.
    console.log(`No TextSlinger runtime for ${target}; skipping. Linux builds cannot be packaged.`);
  } else {
    const runtime = runtimeTarget(target);
    const fetched = await acquireRuntime(runtimeDirectory, runtime);
    console.log(`${fetched ? 'Fetched and verified' : 'Verified'} TextSlinger runtime ${manifest.release} for ${target} (${runtime})`);
  }
}
