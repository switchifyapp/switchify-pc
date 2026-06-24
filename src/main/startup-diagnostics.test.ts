import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { appendStartupDiagnostics, type StartupDiagnosticsEntry } from './startup-diagnostics';

describe('appendStartupDiagnostics', () => {
  it('appends a JSONL diagnostics entry', () => {
    const filePath = diagnosticsPath();

    appendStartupDiagnostics(filePath, entry({ startedAt: '2026-06-24T10:00:00.000Z' }));

    const lines = readLines(filePath);
    expect(lines).toHaveLength(1);
    expect(JSON.parse(lines[0])).toMatchObject({
      startedAt: '2026-06-24T10:00:00.000Z',
      version: '0.1.15',
      startHidden: true
    });
  });

  it('keeps only the newest 50 lines', () => {
    const filePath = diagnosticsPath();

    for (let index = 0; index < 55; index += 1) {
      appendStartupDiagnostics(filePath, entry({ startedAt: `2026-06-24T10:${String(index).padStart(2, '0')}:00.000Z` }));
    }

    const lines = readLines(filePath);
    expect(lines).toHaveLength(50);
    expect(JSON.parse(lines[0]).startedAt).toBe('2026-06-24T10:05:00.000Z');
    expect(JSON.parse(lines[49]).startedAt).toBe('2026-06-24T10:54:00.000Z');
  });

  it('creates the parent directory if it is missing', () => {
    const filePath = join(mkdtempSync(join(tmpdir(), 'switchify-startup-')), 'nested', 'startup.jsonl');

    appendStartupDiagnostics(filePath, entry());

    expect(readLines(filePath)).toHaveLength(1);
  });

  it('drops malformed existing lines without throwing', () => {
    const filePath = diagnosticsPath();
    writeFileSync(filePath, 'not json\n{"startedAt":"old"}\n', 'utf8');

    appendStartupDiagnostics(filePath, entry());

    const lines = readLines(filePath);
    expect(lines).toHaveLength(2);
    expect(JSON.parse(lines[0])).toEqual({ startedAt: 'old' });
  });
});

function diagnosticsPath(): string {
  return join(mkdtempSync(join(tmpdir(), 'switchify-startup-')), 'startup-diagnostics.jsonl');
}

function readLines(filePath: string): string[] {
  return readFileSync(filePath, 'utf8')
    .split(/\r?\n/)
    .filter(Boolean);
}

function entry(overrides: Partial<StartupDiagnosticsEntry> = {}): StartupDiagnosticsEntry {
  return {
    startedAt: '2026-06-24T10:00:00.000Z',
    version: '0.1.15',
    isPackaged: true,
    platform: 'win32',
    executablePath: 'C:\\Program Files\\Switchify PC\\Switchify PC.exe',
    argv: ['Switchify PC.exe', '--start-hidden'],
    startHidden: true,
    startupRegistration: {
      startWithSystem: true,
      registeredCommand: '"C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden',
      startupApproved: 'enabled'
    },
    ...overrides
  };
}
