import { describe, expect, it, vi } from 'vitest';
import { START_HIDDEN_ARG, STARTUP_VALUE_NAME, shouldStartHidden, SystemStartupService } from './system-startup';
import type { StartupRegistryEntry, WindowsStartupRegistry } from './windows-startup-registry';

const expectedCommand = '"C:\\Program Files\\Switchify PC\\Switchify PC.exe" --start-hidden';

describe('shouldStartHidden', () => {
  it('detects the Windows hidden startup argument', () => {
    expect(shouldStartHidden(['Switchify PC.exe', START_HIDDEN_ARG], 'win32')).toBe(true);
  });

  it('ignores the hidden startup argument on non-Windows platforms', () => {
    expect(shouldStartHidden(['Switchify PC', START_HIDDEN_ARG], 'darwin')).toBe(false);
  });

  it('returns false when the hidden startup argument is missing', () => {
    expect(shouldStartHidden(['Switchify PC.exe'], 'win32')).toBe(false);
  });
});

describe('SystemStartupService', () => {
  it('returns unsupported settings on non-Windows platforms without reading the registry', async () => {
    const service = createService({ platform: 'darwin', isPackaged: true });

    await expect(service.getSettings()).resolves.toEqual({
      supported: false,
      startWithSystem: false,
      startsHidden: true,
      reason: 'unsupported_platform'
    });

    await service.setStartWithSystem(true);
    expect(service.startupRegistry.getEntry).not.toHaveBeenCalled();
    expect(service.startupRegistry.setEntry).not.toHaveBeenCalled();
  });

  it('returns unsupported settings for unpackaged Windows builds without writing the registry', async () => {
    const service = createService({ platform: 'win32', isPackaged: false });

    await expect(service.getSettings()).resolves.toEqual({
      supported: false,
      startWithSystem: false,
      startsHidden: true,
      reason: 'unpackaged'
    });

    await service.setStartWithSystem(true);
    expect(service.startupRegistry.setEntry).not.toHaveBeenCalled();
  });

  it('reports enabled startup when the expected command is registered and approved', async () => {
    const service = createService({
      entry: {
        command: expectedCommand,
        startupApproved: 'enabled'
      }
    });

    await expect(service.getSettings()).resolves.toEqual({
      supported: true,
      startWithSystem: true,
      startsHidden: true,
      reason: null,
      registration: {
        expectedCommand,
        registeredCommand: expectedCommand,
        startupApproved: 'enabled'
      }
    });
    expect(service.startupRegistry.getEntry).toHaveBeenCalledWith(STARTUP_VALUE_NAME);
  });

  it('reports enabled startup when StartupApproved is missing but the command matches', async () => {
    const service = createService({
      entry: {
        command: expectedCommand,
        startupApproved: 'missing'
      }
    });

    await expect(service.getSettings()).resolves.toMatchObject({
      supported: true,
      startWithSystem: true,
      registration: {
        startupApproved: 'missing'
      }
    });
  });

  it('reports disabled startup when StartupApproved disables the matching command', async () => {
    const service = createService({
      entry: {
        command: expectedCommand,
        startupApproved: 'disabled'
      }
    });

    await expect(service.getSettings()).resolves.toMatchObject({
      supported: true,
      startWithSystem: false,
      registration: {
        registeredCommand: expectedCommand,
        startupApproved: 'disabled'
      }
    });
  });

  it('reports disabled startup when the Run command is missing', async () => {
    const service = createService({
      entry: {
        command: null,
        startupApproved: 'missing'
      }
    });

    await expect(service.getSettings()).resolves.toMatchObject({
      supported: true,
      startWithSystem: false,
      registration: {
        expectedCommand,
        registeredCommand: null,
        startupApproved: 'missing'
      }
    });
  });

  it('reports disabled startup when the Run command points to an older path', async () => {
    const oldCommand = '"C:\\Old\\Switchify PC.exe" --start-hidden';
    const service = createService({
      entry: {
        command: oldCommand,
        startupApproved: 'enabled'
      }
    });

    await expect(service.getSettings()).resolves.toMatchObject({
      supported: true,
      startWithSystem: false,
      registration: {
        expectedCommand,
        registeredCommand: oldCommand,
        startupApproved: 'enabled'
      }
    });
  });

  it('enables startup by writing the expected command', async () => {
    const service = createService();

    await service.setStartWithSystem(true);

    expect(service.startupRegistry.setEntry).toHaveBeenCalledWith(STARTUP_VALUE_NAME, expectedCommand);
  });

  it('disables startup by deleting the registry entry', async () => {
    const service = createService();

    await service.setStartWithSystem(false);

    expect(service.startupRegistry.deleteEntry).toHaveBeenCalledWith(STARTUP_VALUE_NAME);
  });

  it('returns disabled diagnostics when registry reads fail', async () => {
    const service = createService({ getEntryError: new Error('registry unavailable') });

    await expect(service.getSettings()).resolves.toMatchObject({
      supported: true,
      startWithSystem: false,
      registration: {
        expectedCommand,
        registeredCommand: null,
        startupApproved: 'unknown'
      }
    });
  });
});

function createService(
  options: Partial<{
    platform: NodeJS.Platform;
    isPackaged: boolean;
    entry: StartupRegistryEntry;
    getEntryError: Error;
  }> = {}
): SystemStartupService & {
  startupRegistry: {
    getEntry: ReturnType<typeof vi.fn>;
    setEntry: ReturnType<typeof vi.fn>;
    deleteEntry: ReturnType<typeof vi.fn>;
  };
} {
  const startupRegistry: WindowsStartupRegistry & {
    getEntry: ReturnType<typeof vi.fn>;
    setEntry: ReturnType<typeof vi.fn>;
    deleteEntry: ReturnType<typeof vi.fn>;
  } = {
    getEntry: vi.fn(async () => {
      if (options.getEntryError) throw options.getEntryError;
      return options.entry ?? ({ command: null, startupApproved: 'missing' } satisfies StartupRegistryEntry);
    }),
    setEntry: vi.fn(async () => undefined),
    deleteEntry: vi.fn(async () => undefined)
  };
  const service = new SystemStartupService({
    platform: options.platform ?? 'win32',
    isPackaged: options.isPackaged ?? true,
    executablePath: 'C:\\Program Files\\Switchify PC\\Switchify PC.exe',
    startupRegistry
  });

  return Object.assign(service, { startupRegistry });
}
