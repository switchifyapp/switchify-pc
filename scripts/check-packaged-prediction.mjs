// Verify shipped model/notices independently of the source checkout resources.
import { readdir, readFile } from 'node:fs/promises';
import { join, dirname, resolve } from 'node:path';
import { verified } from './fetch-prediction-model.mjs';
const root = resolve(process.argv[2]);
const files = await readdir(root, { recursive: true });
const models = files.filter(name => name.endsWith('english.sqlite'));
if (models.length !== 1) throw new Error(`Expected one packaged prediction database, found ${models.length}`);
const database = join(root, models[0]);
const manifest = JSON.parse(await readFile(new URL('./prediction-model.json', import.meta.url),'utf8'));
if (!(await verified(dirname(database), manifest))) throw new Error('Packaged prediction files failed verification');
console.log(database);
