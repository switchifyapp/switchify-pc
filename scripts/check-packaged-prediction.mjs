// Verify shipped model/notices independently of the source checkout resources.
import { readdir, readFile } from 'node:fs/promises';
import { join, dirname, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { bundlePins } from './fetch-prediction-neural.mjs';
import { verified } from './fetch-prediction-model.mjs';
const root = resolve(process.argv[2]);
const files = await readdir(root, { recursive: true });
const models = files.filter(name => name.endsWith('english.sqlite'));
if (models.length !== 1) throw new Error(`Expected one packaged prediction database, found ${models.length}`);
const database = join(root, models[0]);
const manifest = JSON.parse(await readFile(new URL('./prediction-model.json', import.meta.url),'utf8'));
if (!(await verified(dirname(database), manifest))) throw new Error('Packaged prediction files failed verification');
const neural = join(dirname(dirname(database)), 'prediction-neural');
if (!(await verified(neural, { files: bundlePins }))) throw new Error('Packaged neural model failed verification');
const pin = JSON.parse(await readFile(new URL('./prediction-neural.json', import.meta.url), 'utf8'));
if (JSON.stringify(JSON.parse(await readFile(join(neural, 'model-bundle.json'), 'utf8'))) !== JSON.stringify(pin.bundle)) throw new Error('Wrong packaged neural policy');
const build = JSON.parse(await readFile(join(neural, 'worker-notices/BUILD.json'), 'utf8'));
if (build.commit !== pin.revision || build.version !== '0.2.0') throw new Error('Wrong worker provenance');
const workerPin = pin.workers[build.target];
if (!workerPin) throw new Error('Unsupported packaged worker');
const windows = build.target.includes('windows');
let portable;
for (const [name, expected] of Object.entries(workerPin.files)) {
  const file = name.split('/').at(-1);
  if (!file.startsWith('switchify-smol-worker')) continue;
  if (file.includes('avx2') && !windows) continue;
  const matches = files.filter(p => p.split(/[\\/]/).at(-1) === file);
  if (matches.length !== 1) throw new Error(`Expected one packaged ${file}`);
  const path = join(root, matches[0]);
  const bytes = await readFile(path);
  // Platform release verification checks signatures after signing changes bytes.
  if (!process.argv.includes('--signed') && (bytes.length !== expected.size || createHash('sha256').update(bytes).digest('hex') !== expected.sha256)) throw new Error('Packaged worker checksum mismatch');
  if (!file.includes('avx2')) portable = path;
}
for (const [name, expected] of Object.entries(workerPin.files)) {
  const file = name.split('/').at(-1);
  if (file.startsWith('switchify-smol-worker')) continue;
  if (!(await verified(join(neural, 'worker-notices'), { files: { [file]: expected } }))) throw new Error('Packaged worker notice mismatch');
}
console.log(process.argv.includes('--worker') ? portable : database);
