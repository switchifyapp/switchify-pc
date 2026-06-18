import { describe, expect, it, vi } from 'vitest';
import { UpdateService, type ElectronUpdaterAdapter } from './update-service';

type Listener = (...args: unknown[]) => void;

class FakeUpdater implements ElectronUpdaterAdapter {
  autoDownload = true;
  autoInstallOnAppQuit = true;
  readonly quitAndInstall = vi.fn();
  readonly listeners = new Map<string, Listener[]>();
  checkForUpdates = vi.fn(async () => null);
  downloadUpdate = vi.fn(async () => []);

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

  it('does not install before an update is downloaded', () => {
    const updater = new FakeUpdater();
    const service = createService({ updater });

    expect(service.installDownloadedUpdate()).toEqual({ ok: false, reason: 'not_downloaded' });
    expect(updater.quitAndInstall).not.toHaveBeenCalled();
  });

  it('calls quitAndInstall after an update is downloaded', async () => {
    const updater = new FakeUpdater();
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-available', updateInfo({ version: '0.1.1' }));
      return null;
    });
    updater.downloadUpdate.mockImplementation(async () => {
      updater.emit('update-downloaded', updateInfo({ version: '0.1.1' }));
      return [];
    });
    const service = createService({ updater });
    await service.checkForUpdates();
    await service.downloadUpdate();

    expect(service.installDownloadedUpdate()).toEqual({ ok: true });
    expect(updater.quitAndInstall).toHaveBeenCalledWith(false, true);
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
  isPackaged = true,
  platform = 'win32'
}: {
  updater?: FakeUpdater;
  isPackaged?: boolean;
  platform?: NodeJS.Platform;
} = {}): UpdateService {
  return new UpdateService({
    currentVersion: '0.1.0',
    isPackaged,
    platform,
    autoUpdater: updater,
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
