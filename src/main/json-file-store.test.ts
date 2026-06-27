import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { backupCorruptJsonFile, writeJsonFileAtomic, writeJsonFileAtomicSync } from './json-file-store';

describe('json file store helpers', () => {
  let tempDir: string | null = null;

  afterEach(() => {
    if (tempDir) {
      rmSync(tempDir, { recursive: true, force: true });
      tempDir = null;
    }
  });

  it('writes async JSON content to the target path', async () => {
    const filePath = path('state.json');

    await writeJsonFileAtomic(filePath, '{ "ok": true }\n');

    expect(readFileSync(filePath, 'utf8')).toBe('{ "ok": true }\n');
    expect(readdirSync(tempDir!).filter((name) => name.endsWith('.tmp'))).toEqual([]);
  });

  it('creates parent directories for async writes', async () => {
    const filePath = path('nested', 'state.json');

    await writeJsonFileAtomic(filePath, '{}\n');

    expect(readFileSync(filePath, 'utf8')).toBe('{}\n');
  });

  it('removes async temp files on failure before rename', async () => {
    const parentFile = path('not-a-directory');
    writeFileSync(parentFile, 'file', 'utf8');

    await expect(writeJsonFileAtomic(join(parentFile, 'state.json'), '{}\n')).rejects.toThrow();
    expect(readdirSync(tempDir!).filter((name) => name.endsWith('.tmp'))).toEqual([]);
  });

  it('writes sync JSON content to the target path', () => {
    const filePath = path('sync-state.json');

    writeJsonFileAtomicSync(filePath, '{ "ok": true }\n');

    expect(readFileSync(filePath, 'utf8')).toBe('{ "ok": true }\n');
    expect(readdirSync(tempDir!).filter((name) => name.endsWith('.tmp'))).toEqual([]);
  });

  it('creates parent directories for sync writes', () => {
    const filePath = path('sync-nested', 'state.json');

    writeJsonFileAtomicSync(filePath, '{}\n');

    expect(readFileSync(filePath, 'utf8')).toBe('{}\n');
  });

  it('backs up corrupt JSON files with a safe filename', async () => {
    const filePath = path('pairing-state.json');
    writeFileSync(filePath, '\0\0', 'utf8');

    const result = await backupCorruptJsonFile(filePath);

    expect(result.backupPath).toMatch(/pairing-state\.corrupt-\d{8}T\d{9}Z\.json$/);
    expect(basename(result.backupPath!)).not.toContain(':');
    expect(readFileSync(result.backupPath!, 'utf8')).toBe('\0\0');
  });

  it('returns null backup path for a missing corrupt file', async () => {
    await expect(backupCorruptJsonFile(path('missing.json'))).resolves.toEqual({ backupPath: null });
  });

  function path(...parts: string[]): string {
    if (!tempDir) {
      tempDir = mkdtempSync(join(tmpdir(), 'switchify-json-store-'));
    }

    return join(tempDir, ...parts);
  }
});
