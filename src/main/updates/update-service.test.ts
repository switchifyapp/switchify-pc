import { afterEach, describe, expect, it, vi } from 'vitest';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  INITIAL_UPDATE_POLL_DELAY_MS,
  UPDATE_POLL_INTERVAL_MS,
  UpdateService,
  type ElectronUpdaterAdapter
} from './update-service';
import type { UpdateInstallerLaunchResult } from './update-installer-launcher';

type Listener = (...args: unknown[]) => void;

class FakeUpdater implements ElectronUpdaterAdapter {
  autoDownload = true;
  autoInstallOnAppQuit = true;
  readonly quitAndInstall = vi.fn();
  readonly listeners = new Map<string, Listener[]>();
  checkForUpdates = vi.fn(async () => null);
  downloadUpdate = vi.fn(async (): Promise<string[]> => []);

  on(event: string, listener: Listener): void {
    const listeners = this.listeners.get(event) ?? [];
    listeners.push(listener);
    this.listeners.set(event, listeners);
  }

  emit(event: string, ...args: unknown[]): void {
    for (const listener of this.listeners.get(event) ?? []) {
      listener(...args);
    }
  }
}

describe('UpdateService', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('disables automatic download and install-on-quit', () => {
    const updater = new FakeUpdater();
    createService({ updater });

    expect(updater.autoDownload).toBe(false);
    expect(updater.autoInstallOnAppQuit).toBe(false);
  });

  it('reports not_packaged in unpackaged builds', async () => {
    const service = createService({ isPackaged: false });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('check_failed');
    expect(state.info.reason).toBe('not_packaged');
  });

  it('reports not_supported on non-Windows platforms', async () => {
    const service = createService({ platform: 'darwin' });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('check_failed');
    expect(state.info.reason).toBe('not_supported');
  });

  it('starts automatic update checks after the initial delay', async () => {
    vi.useFakeTimers();
    const updater = new FakeUpdater();
    const service = createService({ updater });

    service.startAutomaticUpdateChecks();
    await vi.advanceTimersByTimeAsync(INITIAL_UPDATE_POLL_DELAY_MS - 1);
    expect(updater.checkForUpdates).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);

    expect(updater.checkForUpdates).toHaveBeenCalledTimes(1);
  });

  it('checks for updates every hour', async () => {
    vi.useFakeTimers();
    const updater = new FakeUpdater();
    const service = createService({ updater });

    service.startAutomaticUpdateChecks();
    await vi.advanceTimersByTimeAsync(INITIAL_UPDATE_POLL_DELAY_MS);

    expect(updater.checkForUpdates).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(UPDATE_POLL_INTERVAL_MS);

    expect(updater.checkForUpdates).toHaveBeenCalledTimes(2);
  });

  it('does not start automatic checks in unpackaged builds', async () => {
    vi.useFakeTimers();
    const updater = new FakeUpdater();
    const service = createService({ updater, isPackaged: false });

    service.startAutomaticUpdateChecks();
    await vi.advanceTimersByTimeAsync(UPDATE_POLL_INTERVAL_MS * 2);

    expect(updater.checkForUpdates).not.toHaveBeenCalled();
  });

  it('does not start automatic checks on non-Windows platforms', async () => {
    vi.useFakeTimers();
    const updater = new FakeUpdater();
    const service = createService({ updater, platform: 'darwin' });

    service.startAutomaticUpdateChecks();
    await vi.advanceTimersByTimeAsync(UPDATE_POLL_INTERVAL_MS * 2);

    expect(updater.checkForUpdates).not.toHaveBeenCalled();
  });

  it('does not start duplicate polling timers', async () => {
    vi.useFakeTimers();
    const updater = new FakeUpdater();
    const service = createService({ updater });

    service.startAutomaticUpdateChecks();
    service.startAutomaticUpdateChecks();
    await vi.advanceTimersByTimeAsync(UPDATE_POLL_INTERVAL_MS);

    expect(updater.checkForUpdates).toHaveBeenCalledTimes(2);
  });

  it('stops automatic update checks', async () => {
    vi.useFakeTimers();
    const updater = new FakeUpdater();
    const service = createService({ updater });

    service.startAutomaticUpdateChecks();
    service.stopAutomaticUpdateChecks();
    await vi.advanceTimersByTimeAsync(UPDATE_POLL_INTERVAL_MS * 2);

    expect(updater.checkForUpdates).not.toHaveBeenCalled();
  });

  it('automatic checks skip while another operation is active', async () => {
    vi.useFakeTimers();
    const updater = new FakeUpdater();
    let resolveCheck: () => void = () => undefined;
    updater.checkForUpdates.mockImplementation(() => new Promise((resolve) => {
      resolveCheck = () => resolve(null);
    }));
    const service = createService({ updater });

    service.startAutomaticUpdateChecks();
    await vi.advanceTimersByTimeAsync(INITIAL_UPDATE_POLL_DELAY_MS);
    await vi.advanceTimersByTimeAsync(UPDATE_POLL_INTERVAL_MS);

    expect(updater.checkForUpdates).toHaveBeenCalledTimes(1);
    resolveCheck();
  });

  it('automatic checks skip when an update is already downloaded', async () => {
    vi.useFakeTimers();
    const updater = new FakeUpdater();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return ['C:\\cache\\installer.exe'];
    });
    const service = createService({ updater });
    await service.checkForUpdates();
    await service.downloadUpdate();
    updater.checkForUpdates.mockClear();

    service.startAutomaticUpdateChecks();
    await vi.advanceTimersByTimeAsync(UPDATE_POLL_INTERVAL_MS);

    expect(updater.checkForUpdates).not.toHaveBeenCalled();
  });

  it('notifies listeners when update state changes', async () => {
    const updater = new FakeUpdater();
    const onStateChanged = vi.fn();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    const service = createService({ updater, onStateChanged });

    await service.checkForUpdates();

    expect(onStateChanged).toHaveBeenCalledWith(
      expect.objectContaining({
        info: expect.objectContaining({
          status: 'update_available',
          latestVersion: '0.1.1'
        })
      })
    );
  });

  it('maps update-not-available to up_to_date', async () => {
    const updater = new FakeUpdater();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-not-available', updateInfo({ version: '0.1.0' }));
      return null;
    });
    const service = createService({ updater });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('up_to_date');
    expect(state.info.latestVersion).toBe('0.1.0');
  });

  it('maps update-available to update_available', async () => {
    const updater = new FakeUpdater();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1', releaseName: 'v0.1.1' }));
      return null;
    });
    const service = createService({ updater });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('update_available');
    expect(state.info.latestVersion).toBe('0.1.1');
    expect(state.info.releaseName).toBe('v0.1.1');
  });

  it('returns not_available when there is no update to download', async () => {
    const service = createService();

    const state = await service.downloadUpdate();

    expect(state.download.status).toBe('download_failed');
    expect(state.download.reason).toBe('not_available');
  });

  it('maps download progress', async () => {
    const updater = new FakeUpdater();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('download-progress', { transferred: 25, total: 100, percent: 25 });
      return [];
    });
    const service = createService({ updater });
    await service.checkForUpdates();

    const state = await service.downloadUpdate();

    expect(state.download.status).toBe('downloading');
    expect(state.download.downloadedBytes).toBe(25);
    expect(state.download.totalBytes).toBe(100);
    expect(state.download.percent).toBe(25);
  });

  it('maps update-downloaded to downloaded', async () => {
    const updater = new FakeUpdater();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('download-progress', { transferred: 100, total: 100, percent: 100 });
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return [];
    });
    const service = createService({ updater });
    await service.checkForUpdates();

    const state = await service.downloadUpdate();

    expect(state.download.status).toBe('downloaded');
    expect(state.download.percent).toBe(100);
  });

  it('does not install before an update is downloaded', async () => {
    const updater = new FakeUpdater();
    const service = createService({ updater });

    await expect(service.installDownloadedUpdate()).resolves.toEqual({ ok: false, reason: 'not_downloaded' });
    expect(updater.quitAndInstall).not.toHaveBeenCalled();
  });

  it('launches the downloaded installer and quits through the app cleanup path', async () => {
    const updater = new FakeUpdater();
    const launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({ ok: true, pid: 1234 }));
    const quitApp = vi.fn();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return ['C:\\Users\\owen\\AppData\\Local\\switchify-pc-updater\\pending\\Switchify-PC-Setup-0.1.1-x64.exe'];
    });
    const service = createService({ updater, launchInstaller, quitApp });
    await service.checkForUpdates();
    await service.downloadUpdate();

    await expect(service.installDownloadedUpdate()).resolves.toEqual({ ok: true });
    expect(launchInstaller).toHaveBeenCalledWith({
      installerPath: 'C:\\Users\\owen\\AppData\\Local\\switchify-pc-updater\\pending\\Switchify-PC-Setup-0.1.1-x64.exe',
      resourcesPath: 'C:\\Program Files\\Switchify PC\\resources'
    });
    expect(quitApp).toHaveBeenCalledTimes(1);
    expect(updater.quitAndInstall).not.toHaveBeenCalled();
  });

  it('uses the first downloaded path when no exe path is returned', async () => {
    const updater = new FakeUpdater();
    const launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({ ok: true, pid: 1234 }));
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return ['C:\\cache\\download.bin'];
    });
    const service = createService({ updater, launchInstaller });
    await service.checkForUpdates();
    await service.downloadUpdate();

    await service.installDownloadedUpdate();

    expect(launchInstaller).toHaveBeenCalledWith({
      installerPath: 'C:\\cache\\download.bin',
      resourcesPath: 'C:\\Program Files\\Switchify PC\\resources'
    });
  });

  it('returns installer_unavailable and keeps running when no installer path was captured', async () => {
    const updater = new FakeUpdater();
    const launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({
      ok: false,
      reason: 'installer_unavailable'
    }));
    const quitApp = vi.fn();
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return [];
    });
    const service = createService({ updater, launchInstaller, quitApp });
    await service.checkForUpdates();
    await service.downloadUpdate();

    await expect(service.installDownloadedUpdate()).resolves.toEqual({ ok: false, reason: 'installer_unavailable' });
    expect(launchInstaller).toHaveBeenCalledWith({
      installerPath: null,
      resourcesPath: 'C:\\Program Files\\Switchify PC\\resources'
    });
    expect(quitApp).not.toHaveBeenCalled();
    error.mockRestore();
  });

  it('returns launcher failures and does not quit the app', async () => {
    const updater = new FakeUpdater();
    const launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({
      ok: false,
      reason: 'installer_launch_failed'
    }));
    const quitApp = vi.fn();
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return ['C:\\cache\\installer.exe'];
    });
    const service = createService({ updater, launchInstaller, quitApp });
    await service.checkForUpdates();
    await service.downloadUpdate();

    await expect(service.installDownloadedUpdate()).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
    expect(quitApp).not.toHaveBeenCalled();
    error.mockRestore();
  });

  it('returns update launcher unavailable and does not quit the app', async () => {
    const updater = new FakeUpdater();
    const launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({
      ok: false,
      reason: 'update_launcher_unavailable'
    }));
    const quitApp = vi.fn();
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return ['C:\\cache\\installer.exe'];
    });
    const service = createService({ updater, launchInstaller, quitApp });
    await service.checkForUpdates();
    await service.downloadUpdate();

    await expect(service.installDownloadedUpdate()).resolves.toEqual({
      ok: false,
      reason: 'update_launcher_unavailable'
    });
    expect(quitApp).not.toHaveBeenCalled();
    error.mockRestore();
  });

  it('returns UAC cancellation and does not quit the app', async () => {
    const updater = new FakeUpdater();
    const launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({
      ok: false,
      reason: 'uac_cancelled'
    }));
    const quitApp = vi.fn();
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return ['C:\\cache\\installer.exe'];
    });
    const service = createService({ updater, launchInstaller, quitApp });
    await service.checkForUpdates();
    await service.downloadUpdate();

    await expect(service.installDownloadedUpdate()).resolves.toEqual({ ok: false, reason: 'uac_cancelled' });
    expect(quitApp).not.toHaveBeenCalled();
    error.mockRestore();
  });

  it('records install diagnostics without full installer paths', async () => {
    const tempDir = mkdtempSync(join(tmpdir(), 'switchify-update-service-'));
    const diagnosticsFilePath = join(tempDir, 'update-install-diagnostics.jsonl');
    const updater = new FakeUpdater();
    const launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({
      ok: false,
      reason: 'uac_cancelled'
    }));
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return ['C:\\cache\\installer.exe'];
    });
    const service = createService({ updater, launchInstaller, diagnosticsFilePath });
    await service.checkForUpdates();
    await service.downloadUpdate();

    await service.installDownloadedUpdate();

    const log = readFileSync(diagnosticsFilePath, 'utf8');
    expect(log).toContain('"event":"install_requested"');
    expect(log).toContain('"event":"uac_cancelled"');
    expect(log).not.toContain('C:\\cache\\installer.exe');
    error.mockRestore();
    rmSync(tempDir, { recursive: true, force: true });
  });

  it('cleans known updater cache files after successful launcher handoff', async () => {
    const updater = new FakeUpdater();
    const removeFile = vi.fn(async () => undefined);
    const launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({ ok: true, pid: 1234 }));
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return ['C:\\Users\\owen\\AppData\\Local\\switchify-pc-updater\\pending\\Switchify-PC-Setup-0.1.1-x64.exe'];
    });
    const service = createService({ updater, launchInstaller, removeFile });
    await service.checkForUpdates();
    await service.downloadUpdate();

    await service.installDownloadedUpdate();
    await Promise.resolve();

    expect(removeFile).toHaveBeenCalledWith(
      'C:\\Users\\owen\\AppData\\Local\\switchify-pc-updater\\pending\\Switchify-PC-Setup-0.1.1-x64.exe'
    );
    expect(removeFile).toHaveBeenCalledWith(
      'C:\\Users\\owen\\AppData\\Local\\switchify-pc-updater\\pending\\update-info.json'
    );
    expect(removeFile).toHaveBeenCalledWith('C:\\Users\\owen\\AppData\\Local\\switchify-pc-updater\\installer.exe');
  });

  it('maps updater errors during checks to check_failed', async () => {
    const updater = new FakeUpdater();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('error', new Error('failed'));
      return null;
    });
    const service = createService({ updater });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('check_failed');
    expect(state.info.reason).toBe('network_error');
  });

  it('maps updater errors during downloads to download_failed', async () => {
    const updater = new FakeUpdater();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('error', new Error('failed'));
      return [];
    });
    const service = createService({ updater });
    await service.checkForUpdates();

    const state = await service.downloadUpdate();

    expect(state.download.status).toBe('download_failed');
    expect(state.download.reason).toBe('network_error');
  });
});

function createService({
  updater = new FakeUpdater(),
  launchInstaller = vi.fn(async (): Promise<UpdateInstallerLaunchResult> => ({ ok: true, pid: 1234 })),
  quitApp = vi.fn(),
  diagnosticsFilePath = null,
  removeFile = vi.fn(async () => undefined),
  onStateChanged = vi.fn(),
  isPackaged = true,
  platform = 'win32'
}: {
  updater?: FakeUpdater;
  launchInstaller?: typeof import('./update-installer-launcher').launchWindowsUpdateInstaller;
  quitApp?: () => void;
  diagnosticsFilePath?: string | null;
  removeFile?: (path: string) => Promise<void>;
  onStateChanged?: (state: import('../../shared/update').UpdateState) => void;
  isPackaged?: boolean;
  platform?: NodeJS.Platform;
} = {}): UpdateService {
  return new UpdateService({
    currentVersion: '0.1.0',
    isPackaged,
    platform,
    resourcesPath: 'C:\\Program Files\\Switchify PC\\resources',
    autoUpdater: updater,
    launchInstaller,
    quitApp,
    diagnosticsFilePath,
    removeFile,
    onStateChanged,
    now: () => new Date('2026-06-12T12:00:00.000Z')
  });
}

function updateInfo(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: '0.1.0',
    releaseName: null,
    releaseNotes: null,
    ...overrides
  };
}
