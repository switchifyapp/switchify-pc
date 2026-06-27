import { constants } from 'node:fs';
import { mkdir, open, rename, unlink } from 'node:fs/promises';
import { closeSync, existsSync, fsyncSync, mkdirSync, openSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, extname, join, basename } from 'node:path';

export type CorruptJsonBackupResult = {
  backupPath: string | null;
};

export async function writeJsonFileAtomic(filePath: string, content: string): Promise<void> {
  await mkdir(dirname(filePath), { recursive: true });
  const tempPath = tempPathFor(filePath);
  let handle: Awaited<ReturnType<typeof open>> | null = null;
  let renamed = false;

  try {
    handle = await open(tempPath, 'w', 0o600);
    await handle.writeFile(content, 'utf8');
    await handle.sync();
    await handle.close();
    handle = null;
    await rename(tempPath, filePath);
    renamed = true;
  } finally {
    if (handle) {
      await handle.close().catch(() => undefined);
    }

    if (!renamed) {
      await unlink(tempPath).catch(() => undefined);
    }
  }
}

export function writeJsonFileAtomicSync(filePath: string, content: string): void {
  mkdirSync(dirname(filePath), { recursive: true });
  const tempPath = tempPathFor(filePath);
  let fd: number | null = null;
  let renamed = false;

  try {
    fd = openSync(tempPath, constants.O_WRONLY | constants.O_CREAT | constants.O_TRUNC, 0o600);
    writeFileSync(fd, content, 'utf8');
    fsyncSync(fd);
    closeSync(fd);
    fd = null;
    renameSync(tempPath, filePath);
    renamed = true;
  } finally {
    if (fd !== null) {
      try {
        closeSync(fd);
      } catch {
        // Best effort cleanup.
      }
    }

    if (!renamed) {
      try {
        unlinkSync(tempPath);
      } catch {
        // Best effort cleanup.
      }
    }
  }
}

export async function backupCorruptJsonFile(filePath: string): Promise<CorruptJsonBackupResult> {
  if (!existsSync(filePath)) {
    return { backupPath: null };
  }

  const backupPath = corruptBackupPathFor(filePath);

  try {
    await rename(filePath, backupPath);
    return { backupPath };
  } catch (error) {
    if (isMissingFileError(error)) {
      return { backupPath: null };
    }

    throw error;
  }
}

function tempPathFor(filePath: string): string {
  return `${filePath}.${process.pid}.${Date.now()}.${Math.random().toString(16).slice(2)}.tmp`;
}

function corruptBackupPathFor(filePath: string, now = new Date()): string {
  const extension = extname(filePath);
  const name = basename(filePath, extension);
  return join(dirname(filePath), `${name}.corrupt-${sanitizeTimestamp(now.toISOString())}${extension}`);
}

function sanitizeTimestamp(value: string): string {
  return value.replace(/[-:.]/g, '');
}

function isMissingFileError(error: unknown): boolean {
  return (
    error !== null &&
    typeof error === 'object' &&
    'code' in error &&
    (error as { code?: unknown }).code === 'ENOENT'
  );
}
