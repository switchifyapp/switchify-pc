#!/usr/bin/env node
import { generateKeyPairSync } from 'node:crypto';
import { readFile, writeFile, unlink } from 'node:fs/promises';
import { parseArgs } from 'node:util';
import { issueLicence } from './licence.mjs';

async function main() {
  const { values, positionals } = parseArgs({ allowPositionals: true, options: {
    private: { type: 'string' }, public: { type: 'string' }, out: { type: 'string' },
    'key-id': { type: 'string' }, count: { type: 'string', default: '1' }, seats: { type: 'string', default: '1' },
  } });
  if (positionals.length !== 1 || !values.private) throw new Error('See tools/licence-keygen/README.md for usage');
  if (positionals[0] === 'init') {
    if (!values.public) throw new Error('--public is required');
    const { privateKey, publicKey } = generateKeyPairSync('ed25519');
    // Refuse to overwrite either file, including symlinks. Never print secrets.
    await writeFile(values.private, privateKey.export({ type: 'pkcs8', format: 'pem' }), { flag: 'wx', mode: 0o600 });
    try {
      await writeFile(values.public, publicKey.export({ type: 'spki', format: 'pem' }), { flag: 'wx', mode: 0o644 });
    } catch (error) {
      await unlink(values.private);
      throw error;
    }
    return;
  }
  if (positionals[0] !== 'batch' || !values.out) throw new Error('Use init or batch with --out');
  const count = Number(values.count);
  if (!Number.isInteger(count) || count < 1 || count > 10000) throw new Error('--count must be between 1 and 10000');
  const privatePem = await readFile(values.private, 'utf8');
  const rows = Array.from({ length: count }, () => issueLicence(privatePem, { keyId: values['key-id'], seats: Number(values.seats) }));
  const csv = 'licence_id,key\n' + rows.map(row => `${row.licenceId},${row.key}\n`).join('');
  await writeFile(values.out, csv, { flag: 'wx', mode: 0o600 });
}

main().catch(() => { console.error('Licence key generation failed. Check arguments, key format, and output paths. Existing files are never overwritten.'); process.exitCode = 1; });
