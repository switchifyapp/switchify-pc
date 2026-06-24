import { describe, expect, it, vi } from 'vitest';
import {
  createWindowsStartupRegistry,
  startupCommandFor,
  type CommandRunner
} from './windows-startup-registry';

describe('startupCommandFor', () => {
  it('quotes the executable path and appends arguments', () => {
    expect(startupCommandFor('C:\\Program Files\\Switchify PC\\Switchify PC.exe', ['--start-hidden'])).toBe(
      '"C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden'
    );
  });

  it('rejects executable paths containing quotes', () => {
    expect(() => startupCommandFor('C:\\Bad"Path\\Switchify PC.exe', ['--start-hidden'])).toThrow(
      'Startup executable path cannot contain quotes.'
    );
  });

  it('rejects arguments containing quotes', () => {
    expect(() => startupCommandFor('C:\\Switchify PC.exe', ['--bad"arg'])).toThrow(
      'Startup arguments cannot contain quotes.'
    );
  });
});

describe('createWindowsStartupRegistry', () => {
  it('returns command and enabled StartupApproved state', async () => {
    const runner = createRunner({
      runQuery: '    app.switchify.pc    REG_SZ    "C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden',
      approvedQuery: '    app.switchify.pc    REG_BINARY    020000000000000000000000'
    });

    await expect(createWindowsStartupRegistry(runner).getEntry('app.switchify.pc')).resolves.toEqual({
      command: '"C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden',
      startupApproved: 'enabled'
    });
  });

  it('returns null command when the Run value is missing', async () => {
    const runner = createRunner({
      runMissing: true,
      approvedQuery: '    app.switchify.pc    REG_BINARY    020000000000000000000000'
    });

    await expect(createWindowsStartupRegistry(runner).getEntry('app.switchify.pc')).resolves.toEqual({
      command: null,
      startupApproved: 'enabled'
    });
  });

  it('returns missing StartupApproved when that value is missing', async () => {
    const runner = createRunner({
      runQuery: '    app.switchify.pc    REG_SZ    "C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden',
      approvedMissing: true
    });

    await expect(createWindowsStartupRegistry(runner).getEntry('app.switchify.pc')).resolves.toEqual({
      command: '"C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden',
      startupApproved: 'missing'
    });
  });

  it('parses disabled StartupApproved state', async () => {
    const runner = createRunner({
      runQuery: '    app.switchify.pc    REG_SZ    "C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden',
      approvedQuery: '    app.switchify.pc    REG_BINARY    030000000000000000000000'
    });

    await expect(createWindowsStartupRegistry(runner).getEntry('app.switchify.pc')).resolves.toMatchObject({
      startupApproved: 'disabled'
    });
  });

  it('parses unknown StartupApproved state', async () => {
    const runner = createRunner({
      runQuery: '    app.switchify.pc    REG_SZ    "C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden',
      approvedQuery: '    app.switchify.pc    REG_BINARY    090000000000000000000000'
    });

    await expect(createWindowsStartupRegistry(runner).getEntry('app.switchify.pc')).resolves.toMatchObject({
      startupApproved: 'unknown'
    });
  });

  it('writes Run and StartupApproved values when setting an entry', async () => {
    const runner = vi.fn<CommandRunner>(async () => ({ stdout: '', stderr: '' }));

    await createWindowsStartupRegistry(runner).setEntry('app.switchify.pc', '"C:\\Switchify PC.exe" --start-hidden');

    expect(runner).toHaveBeenCalledWith('reg.exe', [
      'add',
      'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run',
      '/v',
      'app.switchify.pc',
      '/t',
      'REG_SZ',
      '/d',
      '"C:\\Switchify PC.exe" --start-hidden',
      '/f'
    ]);
    expect(runner).toHaveBeenCalledWith('reg.exe', [
      'add',
      'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run',
      '/v',
      'app.switchify.pc',
      '/t',
      'REG_BINARY',
      '/d',
      '020000000000000000000000',
      '/f'
    ]);
  });

  it('deletes both Run and StartupApproved values and ignores missing values', async () => {
    const runner = vi.fn<CommandRunner>(async () => {
      throw Object.assign(new Error('The system was unable to find the specified registry key or value.'), {
        code: 1
      });
    });

    await expect(createWindowsStartupRegistry(runner).deleteEntry('app.switchify.pc')).resolves.toBeUndefined();
    expect(runner).toHaveBeenCalledTimes(2);
  });
});

function createRunner(options: {
  runQuery?: string;
  approvedQuery?: string;
  runMissing?: boolean;
  approvedMissing?: boolean;
}): CommandRunner {
  return vi.fn(async (_file, args) => {
    const key = args[1];
    if (key === 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run') {
      if (options.runMissing) throw missingRegistryValueError();
      return { stdout: options.runQuery ?? '', stderr: '' };
    }

    if (key === 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run') {
      if (options.approvedMissing) throw missingRegistryValueError();
      return { stdout: options.approvedQuery ?? '', stderr: '' };
    }

    throw new Error(`Unexpected registry key: ${key}`);
  });
}

function missingRegistryValueError(): Error & { code: number } {
  return Object.assign(new Error('The system was unable to find the specified registry key or value.'), { code: 1 });
}
