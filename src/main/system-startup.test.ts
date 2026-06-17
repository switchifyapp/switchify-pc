import { describe, expect, it, vi } from 'vitest';
import { START_HIDDEN_ARG, shouldStartHidden, SystemStartupService } from './system-startup';

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
  it('returns unsupported settings on non-Windows platforms', () => {
    const service = createService({ platform: 'darwin', isPackaged: true });

    expect(service.getSettings()).toEqual({
      supported: false,
      startWithSystem: false,
      startsHidden: true,
      reason: 'unsupported_platform'
    });

    service.setStartWithSystem(true);
    expect(service.setLoginItemSettings).not.toHaveBeenCalled();
  });

  it('returns unsupported settings for unpackaged Windows builds', () => {
    const service = createService({ platform: 'win32', isPackaged: false });

    expect(service.getSettings()).toEqual({
      supported: false,
      startWithSystem: false,
      startsHidden: true,
      reason: 'unpackaged'
    });

    service.setStartWithSystem(true);
    expect(service.setLoginItemSettings).not.toHaveBeenCalled();
  });

  it('reports enabled startup when the registered executable will launch at login', () => {
    const service = createService({
      loginItemSettings: {
        openAtLogin: true,
        executableWillLaunchAtLogin: true
      }
    });

    expect(service.getSettings()).toEqual({
      supported: true,
      startWithSystem: true,
      startsHidden: true,
      reason: null
    });
    expect(service.getLoginItemSettings).toHaveBeenCalledWith({
      path: 'C:\\Program Files\\Switchify PC\\Switchify PC.exe',
      args: [START_HIDDEN_ARG]
    });
  });

  it('reports disabled startup when Windows will not launch the executable', () => {
    const service = createService({
      loginItemSettings: {
        openAtLogin: true,
        executableWillLaunchAtLogin: false
      }
    });

    expect(service.getSettings()).toMatchObject({
      supported: true,
      startWithSystem: false
    });
  });

  it('enables startup with the hidden startup argument', () => {
    const service = createService();

    service.setStartWithSystem(true);

    expect(service.setLoginItemSettings).toHaveBeenCalledWith({
      openAtLogin: true,
      path: 'C:\\Program Files\\Switchify PC\\Switchify PC.exe',
      args: [START_HIDDEN_ARG],
      name: 'app.switchify.pc',
      enabled: true
    });
  });

  it('disables startup without leaving a disabled login item entry', () => {
    const service = createService();

    service.setStartWithSystem(false);

    expect(service.setLoginItemSettings).toHaveBeenCalledWith({
      openAtLogin: false,
      path: 'C:\\Program Files\\Switchify PC\\Switchify PC.exe',
      args: [START_HIDDEN_ARG],
      name: 'app.switchify.pc'
    });
    expect(service.setLoginItemSettings.mock.calls[0][0]).not.toHaveProperty('enabled');
  });
});

function createService(
  options: Partial<{
    platform: NodeJS.Platform;
    isPackaged: boolean;
    loginItemSettings: {
      openAtLogin: boolean;
      executableWillLaunchAtLogin?: boolean;
    };
  }> = {}
): SystemStartupService & {
  getLoginItemSettings: ReturnType<typeof vi.fn>;
  setLoginItemSettings: ReturnType<typeof vi.fn>;
} {
  const getLoginItemSettings = vi.fn(() => options.loginItemSettings ?? { openAtLogin: false });
  const setLoginItemSettings = vi.fn();
  const service = new SystemStartupService({
    platform: options.platform ?? 'win32',
    isPackaged: options.isPackaged ?? true,
    executablePath: 'C:\\Program Files\\Switchify PC\\Switchify PC.exe',
    appUserModelId: 'app.switchify.pc',
    getLoginItemSettings,
    setLoginItemSettings
  });

  return Object.assign(service, {
    getLoginItemSettings,
    setLoginItemSettings
  });
}
