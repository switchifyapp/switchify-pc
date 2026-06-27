import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { appendUpdateInstallDiagnostic } from './update-install-diagnostics';

const tempDirs: string[] = [];

describe('appendUpdateInstallDiagnostic', () => {
  afterEach(() => {
    for (const dir of tempDirs.splice(0)) {
      rmSync(dir, { recursive: true, force: true });
    }
    vi.restoreAllMocks();
  });

  it('appends a JSONL diagnostics entry', () => {
    const filePath = tempFile();

    appendUpdateInstallDiagnostic(filePath, {
      event: 'install_requested',
      at: '2026-06-27T12:00:00.000Z',
      version: '0.1.18'
    });

    const lines = readFileSync(filePath, 'utf8').trim().split(/\r?\n/);
    expect(lines).toHaveLength(1);
    expect(JSON.parse(lines[0])).toEqual({
      event: 'install_requested',
      at: '2026-06-27T12:00:00.000Z',
      version: '0.1.18'
    });
  });

  it('keeps only the newest 100 lines', () => {
    const filePath = tempFile();

    for (let index = 0; index < 105; index++) {
      appendUpdateInstallDiagnostic(filePath, {
        event: 'installer_launch_failed',
        at: `2026-06-27T12:00:${String(index).padStart(2, '0')}.000Z`,
        version: '0.1.18',
        reason: String(index)
      });
    }

    const lines = readFileSync(filePath, 'utf8').trim().split(/\r?\n/);
    expect(lines).toHaveLength(100);
    expect(JSON.parse(lines[0]).reason).toBe('5');
    expect(JSON.parse(lines.at(-1) ?? '{}').reason).toBe('104');
  });

  it('does not require sensitive fields', () => {
    const filePath = tempFile();

    appendUpdateInstallDiagnostic(filePath, {
      event: 'uac_cancelled',
      at: '2026-06-27T12:00:00.000Z',
      version: '0.1.18'
    });

    const entry = JSON.parse(readFileSync(filePath, 'utf8').trim()) as Record<string, unknown>;
    expect(entry).not.toHaveProperty('installerPath');
    expect(entry).not.toHaveProperty('thumbprint');
  });

  it('swallows write failures with a concise warning', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);

    appendUpdateInstallDiagnostic('', {
      event: 'install_requested',
      at: '2026-06-27T12:00:00.000Z',
      version: '0.1.18'
    });

    expect(warn).toHaveBeenCalled();
  });
});

function tempFile(): string {
  const dir = mkdtempSync(join(tmpdir(), 'switchify-update-diagnostics-'));
  tempDirs.push(dir);
  return join(dir, 'nested', 'update-install-diagnostics.jsonl');
}
