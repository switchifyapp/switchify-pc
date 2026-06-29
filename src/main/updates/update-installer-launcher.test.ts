import { describe, expect, it, vi } from 'vitest';
import { launchWindowsUpdateInstaller, type OpenPath } from './update-installer-launcher';

describe('launchWindowsUpdateInstaller', () => {
  it('returns installer_unavailable when installer path is null', async () => {
    await expect(launchWindowsUpdateInstaller({ installerPath: null })).resolves.toEqual({
      ok: false,
      reason: 'installer_unavailable'
    });
  });

  it('returns installer_unavailable when installer file is missing', async () => {
    await expect(
      launchWindowsUpdateInstaller({
        installerPath: 'C:\\cache\\Switchify-PC-Setup.exe',
        fileExists: () => false
      })
    ).resolves.toEqual({ ok: false, reason: 'installer_unavailable' });
  });

  it('opens the installer path through shell.openPath', async () => {
    const openPath = vi.fn<OpenPath>(async () => '');

    await launchWindowsUpdateInstaller({
      installerPath: 'C:\\cache\\Switchify-PC-Setup.exe',
      openPath,
      fileExists: () => true
    });

    expect(openPath).toHaveBeenCalledWith('C:\\cache\\Switchify-PC-Setup.exe');
  });

  it('returns ok true when openPath resolves to an empty string', async () => {
    await expect(
      launchWindowsUpdateInstaller({
        installerPath: 'C:\\cache\\Switchify-PC-Setup.exe',
        openPath: async () => '',
        fileExists: () => true
      })
    ).resolves.toEqual({ ok: true });
  });

  it('returns installer_launch_failed when openPath resolves to a non-empty error string', async () => {
    await expect(
      launchWindowsUpdateInstaller({
        installerPath: 'C:\\cache\\Switchify-PC-Setup.exe',
        openPath: async () => 'Access denied',
        fileExists: () => true
      })
    ).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
  });

  it('returns installer_launch_failed when openPath rejects', async () => {
    await expect(
      launchWindowsUpdateInstaller({
        installerPath: 'C:\\cache\\Switchify-PC-Setup.exe',
        openPath: async () => {
          throw new Error('failed');
        },
        fileExists: () => true
      })
    ).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
  });

  it('does not require resourcesPath or a packaged launcher', async () => {
    const openPath = vi.fn<OpenPath>(async () => '');

    await expect(
      launchWindowsUpdateInstaller({
        installerPath: 'C:\\cache\\Switchify-PC-Setup.exe',
        openPath,
        fileExists: () => true
      })
    ).resolves.toEqual({ ok: true });
  });
});
